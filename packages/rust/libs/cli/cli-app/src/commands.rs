//! Per-command dispatch and presentation for commands coordinated by the app.

pub(crate) mod config;
pub(crate) mod disk;
pub(crate) mod init;
pub(crate) mod review;
pub(crate) mod spec;
pub(crate) mod sync;

/// Assert that a JSON document carries no ANSI escape sequences.
#[cfg(test)]
pub(crate) fn assert_no_ansi(value: &serde_json::Value) {
    let serialized = serde_json::to_string(value).expect("JSON");
    assert!(!serialized.contains("\\u001b"), "ANSI leaked into JSON");
}
