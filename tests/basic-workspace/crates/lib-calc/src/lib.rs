//! Calculation library demonstrating dependency usage and doctests.

use lib_core::base_offset;

/// Computes the sum of two integers plus the base offset.
///
/// # Examples
///
/// ```
/// use lib_calc::compute_sum;
/// assert_eq!(compute_sum(10, 20), 130);
/// ```
pub fn compute_sum(a: i64, b: i64) -> i64 {
    a + b + base_offset()
}

/// Computes the product of two integers.
///
/// # Examples
///
/// ```
/// use lib_calc::compute_product;
/// assert_eq!(compute_product(5, 6), 30);
/// ```
pub fn compute_product(a: i64, b: i64) -> i64 {
    a * b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calc() {
        assert_eq!(compute_sum(10, 20), 130);
        assert_eq!(compute_product(5, 6), 30);
    }
}
