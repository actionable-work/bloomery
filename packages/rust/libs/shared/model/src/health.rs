use crate::Diagnostic;
use std::fs;
use std::path::{Component, Path};
use toml::Value;

pub const DEFAULT_BASELINE: &str = ".bloomery/code-health-summary.json";

#[derive(Debug, Clone, PartialEq)]
pub struct HealthConfig {
    pub enabled: bool,
    pub baseline: String,
    /// Score floors and regressions in hundredths of a point.
    pub min_crate_score: u16,
    pub min_workspace_score: u16,
    pub max_regression: u16,
    pub cyclomatic_limit: usize,
    pub function_tokens_limit: usize,
    pub nesting_limit: usize,
    pub duplication_free_ratio: f64,
    pub duplicate_min_tokens: usize,
    pub max_cyclomatic: usize,
    pub max_function_tokens: usize,
    pub max_nesting: usize,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            baseline: DEFAULT_BASELINE.to_owned(),
            min_crate_score: 8_000,
            min_workspace_score: 8_000,
            max_regression: 0,
            cyclomatic_limit: 10,
            function_tokens_limit: 250,
            nesting_limit: 4,
            duplication_free_ratio: 0.02,
            duplicate_min_tokens: 64,
            max_cyclomatic: 40,
            max_function_tokens: 1_000,
            max_nesting: 8,
        }
    }
}

impl HealthConfig {
    pub fn from_raw(raw: Option<&Value>) -> Result<Self, Diagnostic> {
        let Some(table) = raw
            .and_then(Value::as_table)
            .and_then(|root| root.get("checks"))
            .and_then(Value::as_table)
            .and_then(|checks| checks.get("codeHealth"))
        else {
            return Ok(Self::default());
        };
        let values = table.as_table().ok_or_else(|| {
            Diagnostic::new("ConfigurationError", "checks.codeHealth must be a table")
        })?;
        let defaults = Self::default();
        let mut config = defaults;
        for (key, value) in values {
            match key.as_str() {
                "enable" => config.enabled = value.as_bool().expect("catalog validates boolean"),
                "baseline" => {
                    config.baseline = value.as_str().expect("catalog validates path").to_owned()
                }
                "minCrateScore" => config.min_crate_score = score_hundredths(value, key)?,
                "minWorkspaceScore" => config.min_workspace_score = score_hundredths(value, key)?,
                "maxRegression" => config.max_regression = score_hundredths(value, key)?,
                "cyclomaticLimit" => config.cyclomatic_limit = positive(value),
                "functionTokensLimit" => config.function_tokens_limit = positive(value),
                "nestingLimit" => config.nesting_limit = positive(value),
                "duplicationFreeRatio" => config.duplication_free_ratio = number(value),
                "duplicateMinTokens" => config.duplicate_min_tokens = positive(value),
                "maxCyclomatic" => config.max_cyclomatic = positive(value),
                "maxFunctionTokens" => config.max_function_tokens = positive(value),
                "maxNesting" => config.max_nesting = positive(value),
                _ => unreachable!("catalog validates known code health keys"),
            }
        }
        if config.min_crate_score > 10_000 || config.min_workspace_score > 10_000 {
            return Err(config_error("health score floors must be from 0 to 100"));
        }
        if config.max_regression > 10_000 {
            return Err(config_error(
                "checks.codeHealth.maxRegression must be from 0 to 100",
            ));
        }
        if !config.duplication_free_ratio.is_finite()
            || !(0.0..1.0).contains(&config.duplication_free_ratio)
        {
            return Err(config_error(
                "checks.codeHealth.duplicationFreeRatio must be finite and from 0 inclusive to 1 exclusive",
            ));
        }
        if config.duplicate_min_tokens < 16 {
            return Err(config_error(
                "checks.codeHealth.duplicateMinTokens must be at least 16",
            ));
        }
        if config.max_cyclomatic < config.cyclomatic_limit
            || config.max_function_tokens < config.function_tokens_limit
            || config.max_nesting < config.nesting_limit
        {
            return Err(config_error(
                "code health hard limits must be at least their corresponding soft limits",
            ));
        }
        validate_baseline(&config.baseline)?;
        Ok(config)
    }

    /// Validate repository containment and protected destinations before a
    /// check formatter or analyzer can run.
    pub fn validate_destination(&self, workspace_root: &Path) -> Result<(), Diagnostic> {
        let root = fs::canonicalize(workspace_root)
            .map_err(|error| config_error(format!("unable to resolve repository root: {error}")))?;
        let relative = Path::new(&self.baseline);
        let destination = root.join(relative);
        let mut current = root.clone();
        let components = relative.components().collect::<Vec<_>>();
        for (index, component) in components.iter().enumerate() {
            current.push(component.as_os_str());
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    if index + 1 == components.len() {
                        return Err(config_error(
                            "checks.codeHealth.baseline must not name a symlink",
                        ));
                    }
                    let resolved = fs::canonicalize(&current).map_err(|error| {
                        config_error(format!("unable to resolve baseline path symlink: {error}"))
                    })?;
                    if !resolved.starts_with(&root) {
                        return Err(config_error(
                            "checks.codeHealth.baseline resolves outside the repository",
                        ));
                    }
                }
                Ok(metadata) if index + 1 < components.len() && !metadata.is_dir() => {
                    return Err(config_error(
                        "checks.codeHealth.baseline has a non-directory parent component",
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                Err(error) => {
                    return Err(config_error(format!(
                        "unable to inspect checks.codeHealth.baseline: {error}"
                    )));
                }
            }
        }
        let mut protected = vec![
            root.join(".bloomery/config.toml"),
            root.join(".bloomery/specs"),
            root.join("Cargo.toml"),
            root.join("Cargo.lock"),
            root.join("flake.lock"),
        ];
        let resolved_destination = if destination.exists() {
            fs::canonicalize(&destination).unwrap_or(destination.clone())
        } else {
            destination.clone()
        };
        for path in &mut protected {
            if path.exists() {
                *path = fs::canonicalize(&*path).unwrap_or_else(|_| path.clone());
            }
        }
        if protected.iter().any(|path| {
            resolved_destination.starts_with(path) || path.starts_with(&resolved_destination)
        }) {
            return Err(config_error(
                "checks.codeHealth.baseline overlaps configuration, specifications, manifests, or lockfiles",
            ));
        }
        Ok(())
    }
}

fn number(value: &Value) -> f64 {
    value
        .as_float()
        .or_else(|| value.as_integer().map(|value| value as f64))
        .expect("catalog validates numeric values")
}

fn positive(value: &Value) -> usize {
    value.as_integer().expect("catalog validates integer") as usize
}

fn score_hundredths(value: &Value, key: &str) -> Result<u16, Diagnostic> {
    let score = number(value);
    let rounded = (score * 100.0).round();
    if !score.is_finite()
        || !(0.0..=100.0).contains(&score)
        || (score * 100.0 - rounded).abs() > 1e-7
    {
        return Err(config_error(format!(
            "checks.codeHealth.{key} must be from 0 to 100 with at most two decimal places"
        )));
    }
    Ok(rounded as u16)
}

fn validate_baseline(value: &str) -> Result<(), Diagnostic> {
    let path = Path::new(value);
    if value.is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || path.file_name().is_none()
    {
        return Err(config_error(
            "checks.codeHealth.baseline must be a repository-relative path without '.' or '..' components",
        ));
    }
    Ok(())
}

fn config_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("ConfigurationError", message)
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_BASELINE, HealthConfig};

    fn parse(extra: &str) -> Result<HealthConfig, super::Diagnostic> {
        let raw =
            toml::from_str::<toml::Value>(&format!("[checks.codeHealth]\n{extra}")).expect("TOML");
        HealthConfig::from_raw(Some(&raw))
    }

    #[test]
    fn defaults_are_available_without_a_health_table() {
        let config = HealthConfig::from_raw(None).expect("defaults");
        assert!(!config.enabled);
        assert_eq!(config.baseline, DEFAULT_BASELINE);
        assert_eq!(config.min_crate_score, 8_000);
        assert_eq!(config.duplication_free_ratio, 0.02);
    }

    #[test]
    fn accepts_fractional_scores_and_disabled_valid_policy() {
        let config = parse("enable = false\nminCrateScore = 74.25\nmaxRegression = 1.5")
            .expect("valid thresholds");
        assert!(!config.enabled);
        assert_eq!(config.min_crate_score, 7_425);
        assert_eq!(config.max_regression, 150);
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-HEALTH-004"))]
    fn rejects_score_precision_and_ratio_boundaries() {
        assert!(parse("minCrateScore = 75.001").is_err());
        assert!(parse("duplicationFreeRatio = 1.0").is_err());
        assert!(parse("duplicateMinTokens = 15").is_err());
        assert!(parse("maxCyclomatic = 9").is_err());
        assert!(parse("baseline = '../outside.json'").is_err());
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-HEALTH-004"))]
    fn baseline_destination_rejects_protected_paths_and_symlink_escapes() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let root = std::env::temp_dir().join(format!(
            "bloomery-health-destination-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(root.join(".bloomery/specs")).expect("managed paths");
        fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("manifest");

        let manifest = HealthConfig {
            baseline: "Cargo.toml".to_owned(),
            ..HealthConfig::default()
        };
        assert!(manifest.validate_destination(&root).is_err());
        let specs = HealthConfig {
            baseline: ".bloomery/specs/health.json".to_owned(),
            ..HealthConfig::default()
        };
        assert!(specs.validate_destination(&root).is_err());

        let outside = root.with_extension("outside");
        fs::create_dir_all(&outside).expect("outside directory");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, root.join("linked")).expect("directory symlink");
            let escaped = HealthConfig {
                baseline: "linked/health.json".to_owned(),
                ..HealthConfig::default()
            };
            assert!(escaped.validate_destination(&root).is_err());
        }
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }
}
