use crate::Diagnostic;
use toml::Value;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClippySuppressionConfig {
    pub enabled: bool,
    pub allow: Vec<String>,
}

impl ClippySuppressionConfig {
    pub fn from_raw(raw: Option<&Value>) -> Result<Self, Diagnostic> {
        let (enabled, allow) =
            crate::policy::enabled_string_list(raw, "clippySuppressions", "allow")?;
        Ok(Self { enabled, allow })
    }
}

#[cfg(test)]
mod tests {
    use super::ClippySuppressionConfig;
    use toml::Value;

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-SUPPRESSIONS-002"))]
    fn suppression_policy_defaults_off_with_empty_allowlist() {
        let config = ClippySuppressionConfig::from_raw(None).expect("default policy");
        assert!(!config.enabled);
        assert!(config.allow.is_empty());
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-SUPPRESSIONS-002"))]
    fn suppression_policy_reads_enabled_and_allowlist() {
        let raw: Value = toml::from_str(
            "[checks.clippySuppressions]\nenable = true\nallow = [\"clippy::unwrap_used\"]\n",
        )
        .expect("TOML");
        let config = ClippySuppressionConfig::from_raw(Some(&raw)).expect("valid policy");
        assert!(config.enabled);
        assert_eq!(config.allow, ["clippy::unwrap_used"]);
    }
}
