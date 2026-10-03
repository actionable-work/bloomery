use std::collections::BTreeMap;

pub const OUTPUT_BYTE_LIMIT: usize = 8 * 1024;
pub const FAILURE_PAGE_LIMIT: usize = bloomery_cli_types::DEFAULT_PAGE_LIMIT;
pub const DISPLAY_RECORD_BYTE_LIMIT: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Passed,
    Failed,
    Blocked,
    Canceled,
    NotRun,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
            Self::Canceled => "canceled",
            Self::NotRun => "not_run",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "passed" => Some(Self::Passed),
            "failed" => Some(Self::Failed),
            "blocked" => Some(Self::Blocked),
            "canceled" => Some(Self::Canceled),
            "not_run" => Some(Self::NotRun),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Passed,
    Failed,
    Error,
    Interrupted,
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Error => "error",
            Self::Interrupted => "interrupted",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "passed" => Some(Self::Passed),
            "failed" => Some(Self::Failed),
            "error" => Some(Self::Error),
            "interrupted" => Some(Self::Interrupted),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureLocation {
    pub path: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureRecord {
    pub id: String,
    pub check: String,
    pub code: String,
    pub subject: Option<String>,
    pub location: Option<FailureLocation>,
    pub message: String,
    pub notes: Vec<String>,
    pub log: Option<String>,
    pub nix_log: Option<String>,
    pub focus_tail: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckRecord {
    pub id: String,
    pub outcome: Outcome,
    pub blocked_by: Option<String>,
    pub logs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSelection {
    pub selectors: Vec<String>,
    pub systems: Vec<String>,
    pub selected_checks: Vec<String>,
    pub partial: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRecord {
    pub id: String,
    pub root: String,
    pub started_at: u64,
    pub completed_at: u64,
    pub source_revision: Option<String>,
    pub source_dirty: Option<bool>,
    pub status: RunStatus,
    pub selection: RunSelection,
    pub outcomes: Vec<CheckRecord>,
    pub failures: Vec<FailureRecord>,
    pub notices: Vec<Notice>,
}

impl RunRecord {
    pub fn counts(&self) -> BTreeMap<&'static str, usize> {
        let mut counts = BTreeMap::new();
        for outcome in &self.outcomes {
            let name = outcome.outcome.as_str();
            *counts.entry(name).or_insert(0) += 1;
        }
        counts
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageWindow {
    pub offset: usize,
    pub end: usize,
    pub next: Option<usize>,
}

pub fn page_window(total: usize, requested_offset: usize, limit: usize) -> PageWindow {
    let offset = requested_offset.min(total);
    let end = offset.saturating_add(limit).min(total);
    PageWindow {
        offset,
        end,
        next: (end < total).then_some(end),
    }
}

pub fn glob_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let value = value.chars().collect::<Vec<_>>();
    let mut matches = vec![false; value.len() + 1];
    matches[0] = true;

    for token in pattern {
        let mut next = vec![false; value.len() + 1];
        match token {
            '*' => {
                next[0] = matches[0];
                for index in 1..=value.len() {
                    next[index] = matches[index] || next[index - 1];
                }
            }
            '?' => next[1..].copy_from_slice(&matches[..value.len()]),
            literal => {
                for index in 1..=value.len() {
                    next[index] = matches[index - 1] && value[index - 1] == literal;
                }
            }
        }
        matches = next;
    }

    matches[value.len()]
}

pub fn selector_may_match_prefix(pattern: &str, prefix: &str) -> bool {
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

pub fn select_ids(catalog: &[String], selectors: &[String]) -> Result<Vec<String>, String> {
    if selectors.is_empty() {
        return Ok(catalog.to_vec());
    }

    let mut selected = std::collections::BTreeSet::new();
    for selector in selectors {
        let matches = catalog
            .iter()
            .filter(|id| glob_matches(selector, id))
            .collect::<Vec<_>>();
        if matches.is_empty() {
            return Err(format!(
                "check selector '{selector}' matched no available check"
            ));
        }
        selected.extend(matches.into_iter().cloned());
    }
    Ok(selected.into_iter().collect())
}

pub fn truncate_utf8(value: &str, maximum_bytes: usize) -> (String, bool) {
    if value.len() <= maximum_bytes {
        return (value.to_owned(), false);
    }
    if maximum_bytes == 0 {
        return (String::new(), true);
    }
    let marker = "…";
    if maximum_bytes < marker.len() {
        return (".".repeat(maximum_bytes), true);
    }

    let mut end = maximum_bytes - marker.len();
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}{marker}", &value[..end]), true)
}

pub fn valid_system_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

#[cfg(test)]
mod tests {
    use super::{
        glob_matches, page_window, select_ids, selector_may_match_prefix, truncate_utf8,
        valid_system_name,
    };
    use bloomery_test_macros::bloomery;

    #[test]
    #[bloomery("CLI-CHECK-SELECT-002")]
    fn exact_selectors_match_one_complete_catalog_id() {
        assert!(glob_matches("static:structure", "static:structure"));
        assert!(!glob_matches("static:struct", "static:structure"));
        assert!(!glob_matches("Static:structure", "static:structure"));
    }

    #[test]
    #[bloomery("CLI-CHECK-SELECT-003")]
    fn globs_are_anchored_case_sensitive_and_support_only_star_and_question() {
        assert!(glob_matches(
            "nix:*:core:?est",
            "nix:x86_64-linux:core:test"
        ));
        assert!(glob_matches("*", "anything"));
        assert!(glob_matches("a?c", "a/c"));
        assert!(!glob_matches("nix:*", "prefix-nix:system:check"));
        assert!(!glob_matches("a[bc]", "ab"));
        assert!(selector_may_match_prefix("nix:*", "nix:"));
        assert!(selector_may_match_prefix("*", "nix:"));
        assert!(!selector_may_match_prefix("static:*", "nix:"));
    }

    #[test]
    #[bloomery("CLI-CHECK-SELECT-004")]
    #[bloomery("CLI-CHECK-SELECT-005")]
    fn selectors_form_a_deduplicated_union_and_reject_unmatched_patterns() {
        let catalog = vec![
            "nix:x86_64-linux:core:doc".to_owned(),
            "nix:x86_64-linux:core:test".to_owned(),
            "static:structure".to_owned(),
        ];
        assert_eq!(
            select_ids(
                &catalog,
                &["nix:*:core:*".to_owned(), "nix:*:core:test".to_owned()]
            )
            .expect("matching selection"),
            vec![
                "nix:x86_64-linux:core:doc".to_owned(),
                "nix:x86_64-linux:core:test".to_owned(),
            ]
        );
        assert!(select_ids(&catalog, &["missing:*".to_owned()]).is_err());
    }

    #[test]
    #[bloomery("CLI-CHECK-SELECT-016")]
    #[bloomery("CLI-CHECK-DETAIL-016")]
    fn page_windows_clamp_end_offsets_and_advance() {
        assert_eq!(
            page_window(5, 2, 2),
            super::PageWindow {
                offset: 2,
                end: 4,
                next: Some(4),
            }
        );
        assert_eq!(
            page_window(5, 9, 2),
            super::PageWindow {
                offset: 5,
                end: 5,
                next: None,
            }
        );
    }

    #[test]
    #[bloomery("CLI-INTERFACE-FLAGS-009")]
    fn system_names_and_utf8_truncation_are_bounded() {
        assert!(valid_system_name("x86_64-linux"));
        assert!(!valid_system_name("../x86_64-linux"));
        let (value, truncated) = truncate_utf8("café", 4);
        assert!(truncated);
        assert!(value.len() <= 4);
        assert!(value.is_char_boundary(value.len()));
    }
}
