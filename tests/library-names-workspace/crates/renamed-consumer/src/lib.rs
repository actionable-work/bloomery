//! ```
//! assert_eq!(renamed_consumer::answer(), renamed::value());
//! ```
pub fn answer() -> u32 {
    renamed::value()
}
#[cfg(test)]
mod tests {
    #[test]
    fn alias_is_used() {
        assert_eq!(super::answer(), renamed::value());
    }
}
