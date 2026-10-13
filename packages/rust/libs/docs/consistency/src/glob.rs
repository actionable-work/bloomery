use std::fs;
use std::path::Path;

/// Expand repository-relative glob roots to matching files.
///
/// The grammar matches the PARSER scanner globs: `*` and `?` within one path
/// segment, and `**` across zero or more segments. Results are sorted and
/// de-duplicated repository-relative paths.
pub fn expand(root: &Path, roots: &[String]) -> Vec<String> {
    let mut matches = Vec::new();
    for pattern in roots {
        let segments: Vec<&str> = pattern
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect();
        walk(root, root, &segments, &mut matches);
    }
    matches.sort();
    matches.dedup();
    matches
}

fn walk(root: &Path, current: &Path, segments: &[&str], out: &mut Vec<String>) {
    let Some((segment, rest)) = segments.split_first() else {
        if current.is_file()
            && let Ok(relative) = current.strip_prefix(root)
        {
            out.push(relative.to_string_lossy().replace('\\', "/"));
        }
        return;
    };

    if *segment == "**" {
        walk(root, current, rest, out);
        for child in child_directories(current) {
            walk(root, &child, segments, out);
        }
        return;
    }

    if has_meta(segment) {
        for child in child_entries(current) {
            let name = child
                .file_name()
                .map(|name| name.to_string_lossy().to_string());
            if name.is_some_and(|name| segment_match(segment, &name)) {
                walk(root, &child, rest, out);
            }
        }
    } else {
        let next = current.join(segment);
        if next.exists() {
            walk(root, &next, rest, out);
        }
    }
}

fn child_entries(path: &Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = fs::read_dir(path) else {
        return Vec::new();
    };
    entries.flatten().map(|entry| entry.path()).collect()
}

fn child_directories(path: &Path) -> Vec<std::path::PathBuf> {
    child_entries(path)
        .into_iter()
        .filter(|entry| entry.is_dir())
        .collect()
}

fn has_meta(segment: &str) -> bool {
    segment.contains('*') || segment.contains('?')
}

fn segment_match(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let name: Vec<char> = name.chars().collect();
    matches_from(&pattern, &name)
}

fn matches_from(pattern: &[char], name: &[char]) -> bool {
    let Some((head, rest)) = pattern.split_first() else {
        return name.is_empty();
    };
    match head {
        '*' => matches_from(rest, name) || (!name.is_empty() && matches_from(pattern, &name[1..])),
        '?' => !name.is_empty() && matches_from(rest, &name[1..]),
        literal => !name.is_empty() && name[0] == *literal && matches_from(rest, &name[1..]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempRoot;

    #[test]
    fn expands_single_and_double_star_segments() {
        let fixture = TempRoot::new("glob");
        fixture.write("README.md", "# root\n");
        fixture.write("docs/a.md", "# a\n");
        fixture.write("docs/nested/b.md", "# b\n");
        fixture.write("docs/c.txt", "c\n");

        let matches = expand(
            fixture.path(),
            &[
                "README.md".to_string(),
                "docs/*.md".to_string(),
                "docs/**/*.md".to_string(),
            ],
        );
        assert_eq!(matches, vec!["README.md", "docs/a.md", "docs/nested/b.md"]);
    }

    #[test]
    fn question_mark_matches_one_character() {
        assert!(segment_match("a?c", "abc"));
        assert!(!segment_match("a?c", "ac"));
        assert!(segment_match("*", "anything"));
    }
}
