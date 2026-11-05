//! Uses [`actual_library::value`] and [`renamed_other::value`].
//!
//! ```
//! assert_eq!(consumer_library::answer(), actual_library::value() + renamed_other::value());
//! assert_eq!(test_alias::value(), 7);
//! ```
pub fn answer() -> u32 {
    assert_eq!(env!("NAMING_BUILD"), "42");
    assert_eq!(macro_alias::answer!(), 42);
    actual_library::value() + renamed_other::value()
}
#[cfg(test)]
mod tests {
    #[test]
    fn names_are_used() {
        assert_eq!(test_alias::value(), 7);
        assert_eq!(
            super::answer(),
            actual_library::value() + renamed_other::value()
        );
    }
}
