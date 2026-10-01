//! Core utilities for calculation and formatting.

/// Converts a 64-bit integer to a string using `itoa`.
///
/// # Examples
///
/// ```
/// use lib_core::int_to_str;
/// assert_eq!(int_to_str(42), "42");
/// ```
pub fn int_to_str(val: i64) -> String {
    let mut buf = itoa::Buffer::new();
    buf.format(val).to_string()
}

/// Returns the standard base offset.
///
/// # Examples
///
/// ```
/// use lib_core::base_offset;
/// assert_eq!(base_offset(), 100);
/// ```
pub fn base_offset() -> i64 {
    100
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int_to_str() {
        assert_eq!(int_to_str(42), "42");
    }

    #[test]
    fn test_offset() {
        assert_eq!(base_offset(), 100);
    }
}
