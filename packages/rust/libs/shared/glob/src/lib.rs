//! Anchored, case-sensitive glob matching shared by selectors and path filters.
//!
//! The grammar supports `*` (any run of characters) and `?` (exactly one
//! character). [`path_matches`] additionally treats a whole `**` segment as zero
//! or more path segments.

use std::path::Path;

/// Match `value` against `pattern` as a whole, character by character.
///
/// `*` and `?` match any character, including `/`.
pub fn matches(pattern: &str, value: &str) -> bool {
    let value = value.chars().collect::<Vec<_>>();
    let mut states = vec![false; value.len() + 1];
    states[0] = true;

    for token in pattern.chars() {
        let mut next = vec![false; value.len() + 1];
        match token {
            '*' => {
                next[0] = states[0];
                for index in 1..=value.len() {
                    next[index] = states[index] || next[index - 1];
                }
            }
            '?' => next[1..].copy_from_slice(&states[..value.len()]),
            literal => {
                for index in 1..=value.len() {
                    next[index] = states[index - 1] && value[index - 1] == literal;
                }
            }
        }
        states = next;
    }

    states[value.len()]
}

/// Match a `/`-separated `path` against a `/`-separated `pattern`.
///
/// Each pattern segment matches one path segment with [`matches()`]; a `**`
/// segment matches zero or more segments. Leading and trailing `/` are ignored.
pub fn path_matches(pattern: &str, path: &Path) -> bool {
    let pattern_parts: Vec<&str> = pattern.trim_matches('/').split('/').collect();
    let path_string = path
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    let path_parts: Vec<&str> = path_string.trim_matches('/').split('/').collect();
    match_parts(&pattern_parts, &path_parts)
}

fn match_parts(pattern: &[&str], path: &[&str]) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }
    if pattern[0] == "**" {
        return match_parts(&pattern[1..], path)
            || (!path.is_empty() && match_parts(pattern, &path[1..]));
    }
    !path.is_empty() && matches(pattern[0], path[0]) && match_parts(&pattern[1..], &path[1..])
}

/// Whether some value starting with `prefix` could match `pattern`.
///
/// Uses the same `*`/`?` grammar as [`matches()`].
pub fn may_match_prefix(pattern: &str, prefix: &str) -> bool {
    fn can_match(
        pattern: &[char],
        prefix: &[char],
        pattern_index: usize,
        prefix_index: usize,
        memo: &mut [Vec<Option<bool>>],
    ) -> bool {
        if prefix_index == prefix.len() {
            return true;
        }
        if pattern_index == pattern.len() {
            return false;
        }
        if let Some(result) = memo[pattern_index][prefix_index] {
            return result;
        }
        let result = match pattern[pattern_index] {
            '*' => {
                can_match(pattern, prefix, pattern_index + 1, prefix_index, memo)
                    || can_match(pattern, prefix, pattern_index, prefix_index + 1, memo)
            }
            '?' => can_match(pattern, prefix, pattern_index + 1, prefix_index + 1, memo),
            character if character == prefix[prefix_index] => {
                can_match(pattern, prefix, pattern_index + 1, prefix_index + 1, memo)
            }
            _ => false,
        };
        memo[pattern_index][prefix_index] = Some(result);
        result
    }

    let pattern = pattern.chars().collect::<Vec<_>>();
    let prefix = prefix.chars().collect::<Vec<_>>();
    let mut memo = vec![vec![None; prefix.len() + 1]; pattern.len() + 1];
    can_match(&pattern, &prefix, 0, 0, &mut memo)
}

#[cfg(test)]
mod tests {
    use super::{matches, path_matches};
    use std::path::Path;

    #[test]
    fn character_globs_are_anchored_and_case_sensitive() {
        assert!(matches("a?c", "abc"));
        assert!(!matches("a?c", "ac"));
        assert!(matches("*", "anything"));
        assert!(matches("*", ""));
        assert!(matches("a*", "a/b"));
        assert!(!matches("A*", "abc"));
        assert!(!matches("b*", "abc"));
        assert!(!matches("a[bc]", "ab"));
    }

    #[test]
    fn path_globs_match_double_star_across_segments() {
        assert!(path_matches("docs/**/*.md", Path::new("docs/a.md")));
        assert!(path_matches("docs/**/*.md", Path::new("docs/nested/b.md")));
        assert!(!path_matches("docs/*.md", Path::new("docs/nested/b.md")));
        assert!(path_matches("/src/", Path::new("src")));
    }
}
