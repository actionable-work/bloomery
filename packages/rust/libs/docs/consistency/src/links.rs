use crate::manifest::Manifest;
use bloomery_model::Diagnostic;
use std::fs;
use std::path::Path;

/// Validate repository-relative Markdown links in every registered surface.
pub fn check(root: &Path, manifest: &Manifest) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for surface in &manifest.surfaces {
        let source = root.join(&surface.path);
        let Ok(contents) = fs::read_to_string(&source) else {
            continue;
        };
        for (index, line) in contents.lines().enumerate() {
            let line_number = index + 1;
            for target in extract_links(line) {
                if is_external(&target) {
                    continue;
                }
                validate_target(root, &source, &target, line_number, &mut diagnostics);
            }
        }
    }
    diagnostics
}

fn validate_target(
    root: &Path,
    source: &Path,
    target: &str,
    line: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (path_part, anchor) = match target.split_once('#') {
        Some((path, anchor)) => (path, Some(anchor)),
        None => (target, None),
    };

    if path_part.is_empty() {
        if let Some(anchor) = anchor
            && !anchor.is_empty()
            && !has_anchor(source, anchor)
        {
            diagnostics.push(
                Diagnostic::new(
                    "DanglingDocumentationAnchor",
                    format!("link target '#{anchor}' names no heading in this document"),
                )
                .at(source.to_path_buf(), Some(line)),
            );
        }
        return;
    }

    if path_part.starts_with("//") || path_part.contains("://") {
        return;
    }

    let relative = path_part.strip_prefix("./").unwrap_or(path_part);
    let absolute = root.join(relative);
    if !absolute.exists() {
        diagnostics.push(
            Diagnostic::new(
                "DanglingDocumentationLink",
                format!("link target '{path_part}' does not exist"),
            )
            .at(source.to_path_buf(), Some(line)),
        );
        return;
    }

    if let Some(anchor) = anchor
        && !anchor.is_empty()
        && absolute.is_file()
        && !has_anchor(&absolute, anchor)
    {
        diagnostics.push(
            Diagnostic::new(
                "DanglingDocumentationAnchor",
                format!("link target '{path_part}#{anchor}' names no heading in '{path_part}'"),
            )
            .at(source.to_path_buf(), Some(line)),
        );
    }
}

fn extract_links(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut links = Vec::new();
    let mut index = 0;
    while index + 1 < bytes.len() {
        if bytes[index] == b']' && bytes[index + 1] == b'(' {
            let start = index + 2;
            if let Some(end) = line[start..].find(')') {
                let raw = line[start..start + end].trim();
                let target = raw
                    .trim_start_matches('<')
                    .trim_end_matches('>')
                    .split_whitespace()
                    .next()
                    .unwrap_or("");
                if !target.is_empty() {
                    links.push(target.to_string());
                }
                index = start + end + 1;
                continue;
            }
        }
        index += 1;
    }
    links
}

fn is_external(target: &str) -> bool {
    target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:")
        || target.starts_with("tel:")
        || target.starts_with("ftp://")
        || target.starts_with("www.")
}

fn has_anchor(path: &Path, anchor: &str) -> bool {
    let Ok(contents) = fs::read_to_string(path) else {
        return false;
    };
    let mut occurrences = std::collections::BTreeMap::<String, usize>::new();
    for line in contents.lines() {
        let trimmed = line.trim_start();
        let hash_count = trimmed
            .chars()
            .take_while(|character| *character == '#')
            .count();
        if (1..=6).contains(&hash_count)
            && trimmed
                .chars()
                .nth(hash_count)
                .is_some_and(char::is_whitespace)
        {
            let heading = trimmed[hash_count..].trim().trim_end_matches('#').trim();
            let base = slugify(heading);
            let count = occurrences.entry(base.clone()).or_insert(0);
            let candidate = if *count == 0 {
                base
            } else {
                format!("{base}-{}", *count)
            };
            *count += 1;
            if candidate == anchor {
                return true;
            }
        }
    }
    false
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut pending_dash = false;
    for character in value.chars().flat_map(|character| character.to_lowercase()) {
        if character.is_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            pending_dash = false;
            slug.push(character);
        } else if !slug.is_empty() {
            pending_dash = true;
        }
    }
    slug
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempRoot;

    fn manifest(path: &str) -> Manifest {
        Manifest {
            roots: vec![path.to_string()],
            surfaces: vec![crate::Surface {
                path: path.to_string(),
                features: Vec::new(),
            }],
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-CONSISTENCY-LINKS-001"))]
    fn missing_targets_are_reported() {
        let fixture = TempRoot::new("links-missing");
        fixture.write("README.md", "[missing](./does-not-exist.md)\n");

        let diagnostics = check(fixture.path(), &manifest("README.md"));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "DanglingDocumentationLink"),
            "{diagnostics:?}"
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-CONSISTENCY-LINKS-002"))]
    fn missing_anchors_are_reported() {
        let fixture = TempRoot::new("links-anchor");
        fixture.write("target.md", "# Present\n");
        fixture.write("README.md", "[missing](./target.md#absent)\n");

        let diagnostics = check(fixture.path(), &manifest("README.md"));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "DanglingDocumentationAnchor"),
            "{diagnostics:?}"
        );

        fixture.write("README.md", "[present](./target.md#present)\n");
        let diagnostics = check(fixture.path(), &manifest("README.md"));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-CONSISTENCY-LINKS-003"))]
    fn external_links_are_not_fetched() {
        let fixture = TempRoot::new("links-external");
        fixture.write(
            "README.md",
            "[site](https://example.com/missing) and [mail](mailto:a@example.com)\n",
        );

        let diagnostics = check(fixture.path(), &manifest("README.md"));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }
}
