//! Shared library used by the benchmark binary.

pub use util::decimal;

/// Format a value with the shared prefix.
pub fn label(value: i64) -> String {
    format!("shared:{}", decimal(value))
}
