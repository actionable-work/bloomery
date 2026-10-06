//! Greeting logic for the workspace.

/// Formats a greeting for `name`.
pub fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}

#[cfg(test)]
mod tests {
    use super::greet;

    #[cfg_attr(any(), bloomery("APP-CORE-BEHAVIOR-001"))]
    #[test]
    fn greets_a_name() {
        assert_eq!(greet("world"), "Hello, world!");
    }
}
