//! Small formatting helpers shared across the benchmark workspace.

/// Return the decimal representation of `value`.
pub fn decimal(value: i64) -> String {
    let mut buffer = itoa::Buffer::new();
    buffer.format(value).to_owned()
}

#[cfg(test)]
mod tests {
    use super::decimal;

    #[test]
    fn formats_negative_values() {
        assert_eq!(decimal(-42), "-42");
    }
}
