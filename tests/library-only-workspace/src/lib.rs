pub fn library_only() -> &'static str {
    "library only"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_library_function() {
        assert_eq!(library_only(), "library only");
    }
}
