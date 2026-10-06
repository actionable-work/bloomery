//! Leaf library that combines the shared helpers and the proc-macro.

pub use util::decimal;

/// Return the benchmark constant.
///
/// ```
/// assert_eq!(leaf::answer(), 42);
/// ```
pub fn answer() -> i64 {
    macros::value!()
}
