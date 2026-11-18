pub use bloomery_nix::valid_system_name;
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RunStatus {
    #[default]
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

    /// Process exit code for a run ending with this status.
    pub fn exit_code(self) -> u8 {
        match self {
            Self::Passed => 0,
            Self::Failed => 1,
            Self::Error => 2,
            Self::Interrupted => 130,
        }
    }

    /// JSON envelope status for a run ending with this status.
    pub fn envelope_status(self) -> bloomery_cli_output::Status {
        match self {
            Self::Passed => bloomery_cli_output::Status::Succeeded,
            Self::Failed => bloomery_cli_output::Status::Failed,
            Self::Error => bloomery_cli_output::Status::Error,
            Self::Interrupted => bloomery_cli_output::Status::Interrupted,
        }
    }

    /// Human outcome marker: errors read as failures, interruptions as skips.
    pub fn outcome_status(self) -> bloomery_cli_output::OutcomeStatus {
        match self {
            Self::Passed => bloomery_cli_output::OutcomeStatus::Passed,
            Self::Failed | Self::Error => bloomery_cli_output::OutcomeStatus::Failed,
            Self::Interrupted => bloomery_cli_output::OutcomeStatus::Skipped,
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
pub struct FailureDraft {
    pub check: String,
    pub code: String,
    pub subject: Option<String>,
    pub location: Option<FailureLocation>,
    pub message: String,
    pub notes: Vec<String>,
    pub log: Option<String>,
    pub nix_log: Option<String>,
    pub focus_tail: bool,
    pub occurrence: usize,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Write,
    NoWrite,
}

impl RunMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::NoWrite => "no-write",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "write" => Some(Self::Write),
            "no-write" => Some(Self::NoWrite),
            _ => None,
        }
    }
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
    pub mode: RunMode,
    pub health_baseline_path: Option<String>,
    pub health_baseline_refreshed: bool,
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

#[cfg(test)]
mod tests {
    use super::{page_window, truncate_utf8, valid_system_name};

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-016"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-DETAIL-016"))]
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
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-009"))]
    fn system_names_and_utf8_truncation_are_bounded() {
        assert!(valid_system_name("x86_64-linux"));
        assert!(!valid_system_name("../x86_64-linux"));
        let (value, truncated) = truncate_utf8("café", 4);
        assert!(truncated);
        assert!(value.len() <= 4);
        assert!(value.is_char_boundary(value.len()));
    }
}
