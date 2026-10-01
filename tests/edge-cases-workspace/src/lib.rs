pub fn root_greeting() -> &'static str {
    "hello from root library"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_lib() {
        assert_eq!(root_greeting(), "hello from root library");
    }
}
