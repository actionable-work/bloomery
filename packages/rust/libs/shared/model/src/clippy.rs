use crate::Diagnostic;
use toml::Value;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClippySuppressionConfig {
    pub enabled: bool,
    pub allow: Vec<String>,
}

impl ClippySuppressionConfig {
    pub fn from_raw(raw: Option<&Value>) -> Result<Self, Diagnostic> {
        let Some(table) = raw
            .and_then(Value::as_table)
            .and_then(|root| root.get("checks"))
            .and_then(Value::as_table)
            .and_then(|checks| checks.get("clippySuppressions"))
        else {
            return Ok(Self::default());
        };
        let table = table.as_table().ok_or_else(|| {
            Diagnostic::new(
                "ConfigurationError",
                "checks.clippySuppressions must be a table",
            )
        })?;
        let enabled = table
            .get("enable")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let allow = table
            .get("allow")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
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
