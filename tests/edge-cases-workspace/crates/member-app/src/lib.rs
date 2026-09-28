#[cfg(modern_directive_active)]
pub fn cfg_active() -> bool {
    true
}

#[cfg(not(modern_directive_active))]
pub fn cfg_active() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modern_directive() {
        assert!(cfg_active(), "cargo::rustc-cfg directive must be active");
    }
}
