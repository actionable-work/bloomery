use crate::Diagnostic;
use toml::Value;

/// `[checks.unusedDependencies]` policy consumed by `static:unused-dependencies`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnusedDependencyConfig {
    pub enabled: bool,
    /// Dependency package names exempt from the check.
    pub exceptions: Vec<String>,
}

impl UnusedDependencyConfig {
    pub fn from_raw(raw: Option<&Value>) -> Result<Self, Diagnostic> {
        let (enabled, exceptions) =
            crate::policy::enabled_string_list(raw, "unusedDependencies", "exceptions")?;
        Ok(Self {
            enabled,
            exceptions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::UnusedDependencyConfig;
    use toml::Value;

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-UNUSED-002"))]
    fn unused_dependency_policy_defaults_off_with_no_exceptions() {
        let config = UnusedDependencyConfig::from_raw(None).expect("default policy");
        assert_eq!(config, UnusedDependencyConfig::default());
        let raw: Value = toml::from_str("[checks]\nenable = true\n").expect("TOML");
        let config = UnusedDependencyConfig::from_raw(Some(&raw)).expect("absent table");
        assert!(!config.enabled);
        assert!(config.exceptions.is_empty());
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-UNUSED-002"))]
    fn unused_dependency_policy_reads_enabled_and_exceptions() {
        let raw: Value = toml::from_str(
            "[checks.unusedDependencies]\nenable = true\nexceptions = [\"openssl-sys\"]\n",
        )
        .expect("TOML");
        let config = UnusedDependencyConfig::from_raw(Some(&raw)).expect("valid policy");
        assert!(config.enabled);
        assert_eq!(config.exceptions, ["openssl-sys"]);
    }
}
