use crate::cli_catalog;
use crate::manifest::Manifest;
use bloomery_model::Diagnostic;
use std::fs;
use std::path::Path;

/// Validate role-tagged fenced code blocks in every registered surface.
pub fn check(root: &Path, manifest: &Manifest) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for surface in &manifest.surfaces {
        let path = root.join(&surface.path);
        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };
        for fence in fences(&contents) {
            match fence.role.as_deref() {
                Some("bloomery-config") => validate_config(&path, &fence, &mut diagnostics),
                Some("bloomery-cli") => validate_cli(&path, &fence, &mut diagnostics),
                _ => {}
            }
        }
    }
    diagnostics
}

struct Fence {
    role: Option<String>,
    content: String,
    line: usize,
}

fn fences(contents: &str) -> Vec<Fence> {
    let mut result = Vec::new();
    let mut lines = contents.lines().enumerate();
    while let Some((index, line)) = lines.next() {
        let trimmed = line.trim();
        let Some((marker, count)) = fence_marker(trimmed) else {
            continue;
        };
        let info = trimmed.trim_start_matches(marker).trim();
        let mut tokens = info.split_whitespace();
        let _language = tokens.next();
        let role = tokens.next().map(str::to_owned);

        let mut content = String::new();
        for (_, inner) in lines.by_ref() {
            let inner = inner.trim();
            if inner.len() >= count && inner.chars().all(|character| character == marker) {
                break;
            }
            content.push_str(inner);
            content.push('\n');
        }

        result.push(Fence {
            role,
            content,
            line: index + 1,
        });
    }
    result
}

fn fence_marker(trimmed: &str) -> Option<(char, usize)> {
    let marker = trimmed.chars().next()?;
    if marker != '`' && marker != '~' {
        return None;
    }
    let count = 1 + trimmed.chars().skip(1).take_while(|c| *c == marker).count();
    if count < 3 {
        return None;
    }
    Some((marker, count))
}

fn validate_config(path: &Path, fence: &Fence, diagnostics: &mut Vec<Diagnostic>) {
    let value = match toml::from_str::<toml::Value>(&fence.content) {
        Ok(value) => value,
        Err(_) => {
            diagnostics.push(
                Diagnostic::new(
                    "MalformedConfigExcerpt",
                    "tagged configuration excerpt is not valid TOML",
                )
                .at(path, Some(fence.line)),
            );
            return;
        }
    };

    let mut leaves = Vec::new();
    let mut prefix = Vec::new();
    collect_leaves(&value, &mut prefix, &mut leaves);
    for leaf in leaves {
        if leaf.as_slice() == ["version"] {
            continue;
        }
        let segments: Vec<&str> = leaf.iter().map(String::as_str).collect();
        if bloomery_model::catalog::resolve(&segments).is_none() {
            diagnostics.push(
                Diagnostic::new(
                    "UnknownConfigKeyInDocumentation",
                    format!(
                        "configuration excerpt uses unknown key '{}'",
                        leaf.join(".")
                    ),
                )
                .at(path, Some(fence.line)),
            );
        }
    }
}

fn collect_leaves(value: &toml::Value, prefix: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
    if let toml::Value::Table(table) = value {
        for (key, child) in table {
            prefix.push(key.clone());
            collect_leaves(child, prefix, out);
            prefix.pop();
        }
    } else {
        out.push(prefix.clone());
    }
}

fn validate_cli(path: &Path, fence: &Fence, diagnostics: &mut Vec<Diagnostic>) {
    let catalog = cli_catalog::command_paths();
    for (offset, line) in fence.content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        let command_tokens: Vec<&str> =
            if let Some(position) = tokens.iter().position(|token| *token == "--") {
                tokens[position + 1..].to_vec()
            } else if tokens.first().copied() == Some("bloomery") {
                tokens[1..].to_vec()
            } else {
                continue;
            };

        let command_path: Vec<&str> = command_tokens
            .iter()
            .take_while(|token| !token.starts_with('-'))
            .copied()
            .collect();
        if command_path.is_empty() {
            continue;
        }

        let known = (1..=command_path.len())
            .rev()
            .any(|length| catalog.contains(&command_path[..length].join(" ")));
        if !known {
            diagnostics.push(
                Diagnostic::new(
                    "UnknownCommandInDocumentation",
                    format!(
                        "CLI excerpt uses unknown command '{}'",
                        command_path.join(" ")
                    ),
                )
                .at(path, Some(fence.line + offset)),
            );
        }
    }
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
    #[cfg_attr(any(), bloomery("DOCS-CONSISTENCY-EXCERPTS-001"))]
    fn config_excerpts_use_catalog_keys() {
        let fixture = TempRoot::new("excerpts-config");
        fixture.write(
            "README.md",
            "```toml bloomery-config\nversion = 2\n[build]\nprofileName = \"release\"\n```\n",
        );
        let diagnostics = check(fixture.path(), &manifest("README.md"));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        fixture.write(
            "README.md",
            "```toml bloomery-config\n[build]\nnotAKey = true\n```\n",
        );
        let diagnostics = check(fixture.path(), &manifest("README.md"));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "UnknownConfigKeyInDocumentation"),
            "{diagnostics:?}"
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-CONSISTENCY-EXCERPTS-002"))]
    fn malformed_config_excerpts_are_reported() {
        let fixture = TempRoot::new("excerpts-malformed");
        fixture.write(
            "README.md",
            "```toml bloomery-config\n[build\nprofileName = \"release\"\n```\n",
        );

        let diagnostics = check(fixture.path(), &manifest("README.md"));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "MalformedConfigExcerpt"),
            "{diagnostics:?}"
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-CONSISTENCY-EXCERPTS-003"))]
    fn cli_excerpts_use_catalogued_commands() {
        let fixture = TempRoot::new("excerpts-cli");
        fixture.write(
            "README.md",
            "```bash bloomery-cli\nnix run .#bloomery:dev -- check\nbloomery spec trace DOCS-CONSISTENCY-EXCERPTS-003\n```\n",
        );
        let diagnostics = check(fixture.path(), &manifest("README.md"));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        fixture.write("README.md", "```bash bloomery-cli\nbloomery bogus\n```\n");
        let diagnostics = check(fixture.path(), &manifest("README.md"));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "UnknownCommandInDocumentation"),
            "{diagnostics:?}"
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-CONSISTENCY-EXCERPTS-004"))]
    fn untagged_excerpts_are_ignored() {
        let fixture = TempRoot::new("excerpts-untagged");
        fixture.write(
            "README.md",
            "```toml\n[build]\nnotAKey = true\n```\n```bash\nbloomery bogus\n```\n",
        );

        let diagnostics = check(fixture.path(), &manifest("README.md"));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }
}
