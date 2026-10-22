use crate::Diagnostic;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path};
use toml::Value;

pub const DEFAULT_BASELINE: &str = ".bloomery/code-health-summary.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HealthWeights {
    /// Relative weights in hundredths, from 0 through 10,000.
    pub complexity: u16,
    pub size: u16,
    pub nesting: u16,
    pub duplication: u16,
    pub crate_size: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HealthLimitOverrides {
    pub max_cyclomatic: Option<usize>,
    pub max_function_tokens: Option<usize>,
    pub max_nesting: Option<usize>,
    pub max_crate_sloc: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectiveHardLimits {
    pub max_cyclomatic: usize,
    pub max_function_tokens: usize,
    pub max_nesting: usize,
    pub max_crate_sloc: usize,
}

impl Default for HealthWeights {
    fn default() -> Self {
        Self {
            complexity: 2_500,
            size: 2_000,
            nesting: 1_000,
            duplication: 1_500,
            crate_size: 3_000,
        }
    }
}

impl HealthWeights {
    pub fn total(self) -> u32 {
        u32::from(self.complexity)
            + u32::from(self.size)
            + u32::from(self.nesting)
            + u32::from(self.duplication)
            + u32::from(self.crate_size)
    }
}

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
    pub crate_sloc_limit: usize,
    pub max_crate_sloc: usize,
    pub weights: HealthWeights,
    /// Hard-limit overrides keyed by exact workspace-relative Cargo manifest path.
    pub crate_exceptions: BTreeMap<String, HealthLimitOverrides>,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            baseline: DEFAULT_BASELINE.to_owned(),
            min_crate_score: 8_000,
            min_workspace_score: 8_000,
            max_regression: 200,
            cyclomatic_limit: 10,
            function_tokens_limit: 250,
            nesting_limit: 4,
            duplication_free_ratio: 0.02,
            duplicate_min_tokens: 64,
            max_cyclomatic: 40,
            max_function_tokens: 1_000,
            max_nesting: 8,
            crate_sloc_limit: 1_500,
            max_crate_sloc: 5_000,
            weights: HealthWeights::default(),
            crate_exceptions: BTreeMap::new(),
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
        let mut config = Self::default();
        apply_health_values(&mut config, values)?;
        validate_health_policy(&config)?;
        Ok(config)
    }

    pub fn hard_limits_for_crate(&self, manifest_path: &str) -> EffectiveHardLimits {
        let overrides = self.crate_exceptions.get(manifest_path);
        EffectiveHardLimits {
            max_cyclomatic: overrides
                .and_then(|limits| limits.max_cyclomatic)
                .unwrap_or(self.max_cyclomatic),
            max_function_tokens: overrides
                .and_then(|limits| limits.max_function_tokens)
                .unwrap_or(self.max_function_tokens),
            max_nesting: overrides
                .and_then(|limits| limits.max_nesting)
                .unwrap_or(self.max_nesting),
            max_crate_sloc: overrides
                .and_then(|limits| limits.max_crate_sloc)
                .unwrap_or(self.max_crate_sloc),
        }
    }

    pub fn validate_crate_exception_manifests(
        &self,
        manifest_paths: &[String],
    ) -> Result<(), Diagnostic> {
        for manifest in self.crate_exceptions.keys() {
            if !manifest_paths.iter().any(|candidate| candidate == manifest) {
                return Err(config_error(format!(
                    "checks.codeHealth.crateExceptions.{manifest} does not match a selected workspace crate Cargo.toml"
                )));
            }
        }
        Ok(())
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

fn apply_health_values(
    config: &mut HealthConfig,
    values: &toml::map::Map<String, Value>,
) -> Result<(), Diagnostic> {
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
            "crateSlocLimit" => config.crate_sloc_limit = positive(value),
            "maxCrateSloc" => config.max_crate_sloc = positive(value),
            "weights" => config.weights = parse_weights(value)?,
            "crateExceptions" => config.crate_exceptions = parse_crate_exceptions(value)?,
            _ => unreachable!("catalog validates known code health keys"),
        }
    }
    Ok(())
}

fn validate_health_policy(config: &HealthConfig) -> Result<(), Diagnostic> {
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
        || config.max_crate_sloc < config.crate_sloc_limit
    {
        return Err(config_error(
            "code health hard limits must be at least their corresponding soft limits",
        ));
    }
    validate_crate_exceptions(config)?;
    if config.weights.total() == 0 {
        return Err(config_error(
            "checks.codeHealth.weights must have a positive effective sum",
        ));
    }
    validate_baseline(&config.baseline)
}

fn validate_crate_exceptions(config: &HealthConfig) -> Result<(), Diagnostic> {
    for (manifest, limits) in &config.crate_exceptions {
        validate_manifest_path(manifest)?;
        if limits
            .max_cyclomatic
            .is_some_and(|limit| limit < config.cyclomatic_limit)
            || limits
                .max_function_tokens
                .is_some_and(|limit| limit < config.function_tokens_limit)
            || limits
                .max_nesting
                .is_some_and(|limit| limit < config.nesting_limit)
            || limits
                .max_crate_sloc
                .is_some_and(|limit| limit < config.crate_sloc_limit)
        {
            return Err(config_error(format!(
                "checks.codeHealth.crateExceptions.{manifest} hard limits must be at least their corresponding soft limits"
            )));
        }
    }
    Ok(())
}

fn number(value: &Value) -> f64 {
    value
        .as_float()
        .or_else(|| value.as_integer().map(|value| value as f64))
        .expect("catalog validates numeric values")
}

fn parse_weights(value: &Value) -> Result<HealthWeights, Diagnostic> {
    let table = value
        .as_table()
        .ok_or_else(|| config_error("checks.codeHealth.weights must be a table"))?;
    let mut weights = HealthWeights::default();
    for (key, value) in table {
        let parsed = weight_hundredths(value, key)?;
        match key.as_str() {
            "complexity" => weights.complexity = parsed,
            "size" => weights.size = parsed,
            "nesting" => weights.nesting = parsed,
            "duplication" => weights.duplication = parsed,
            "crateSize" => weights.crate_size = parsed,
            _ => {
                return Err(config_error(format!(
                    "unknown health weight: checks.codeHealth.weights.{key}"
                )));
            }
        }
    }
    if weights.total() == 0 {
        return Err(config_error(
            "checks.codeHealth.weights must have a positive effective sum",
        ));
    }
    Ok(weights)
}

fn parse_crate_exceptions(
    value: &Value,
) -> Result<BTreeMap<String, HealthLimitOverrides>, Diagnostic> {
    let table = value
        .as_table()
        .ok_or_else(|| config_error("checks.codeHealth.crateExceptions must be a table"))?;
    let mut exceptions = BTreeMap::new();
    for (manifest, value) in table {
        validate_manifest_path(manifest)?;
        let values = value.as_table().ok_or_else(|| {
            config_error(format!(
                "checks.codeHealth.crateExceptions.{manifest} must be a table"
            ))
        })?;
        let mut limits = HealthLimitOverrides::default();
        for (key, value) in values {
            let parsed = value
                .as_integer()
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| *value > 0)
                .ok_or_else(|| {
                    config_error(format!(
                        "checks.codeHealth.crateExceptions.{manifest}.{key} must be a positive integer"
                    ))
                })?;
            match key.as_str() {
                "maxCyclomatic" => limits.max_cyclomatic = Some(parsed),
                "maxFunctionTokens" => limits.max_function_tokens = Some(parsed),
                "maxNesting" => limits.max_nesting = Some(parsed),
                "maxCrateSloc" => limits.max_crate_sloc = Some(parsed),
                _ => {
                    return Err(config_error(format!(
                        "unknown code health crate exception key: checks.codeHealth.crateExceptions.{manifest}.{key}"
                    )));
                }
            }
        }
        exceptions.insert(manifest.clone(), limits);
    }
    Ok(exceptions)
}

fn validate_manifest_path(value: &str) -> Result<(), Diagnostic> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path.file_name().is_none_or(|name| name != "Cargo.toml")
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(config_error(format!(
            "checks.codeHealth.crateExceptions.{value} must be a workspace-relative path to Cargo.toml without '.' or '..' components"
        )));
    }
    Ok(())
}

fn weight_hundredths(value: &Value, key: &str) -> Result<u16, Diagnostic> {
    let weight = number(value);
    let rounded = (weight * 100.0).round();
    if !weight.is_finite()
        || !(0.0..=100.0).contains(&weight)
        || (weight * 100.0 - rounded).abs() > 1e-7
    {
        return Err(config_error(format!(
            "checks.codeHealth.weights.{key} must be finite, from 0 to 100, with at most two decimal places"
        )));
    }
    Ok(rounded as u16)
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
        assert_eq!(config.max_regression, 200);
        assert_eq!(config.crate_sloc_limit, 1_500);
        assert_eq!(config.max_crate_sloc, 5_000);
        assert_eq!(config.weights, super::HealthWeights::default());
        assert!(config.crate_exceptions.is_empty());
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
        assert!(parse("crateSlocLimit = 1501\nmaxCrateSloc = 1500").is_err());
        assert!(parse("weights = 5").is_err());
        assert!(parse("[checks.codeHealth.weights]\ncomplexity = 100.001").is_err());
        assert!(parse("[checks.codeHealth.weights]\ncomplexity = -1").is_err());
        assert!(parse("[checks.codeHealth.weights]\ncomplexity = 0\nsize = 0\nnesting = 0\nduplication = 0\ncrateSize = 0").is_err());
        assert!(parse("baseline = '../outside.json'").is_err());
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-HEALTH-005"))]
    fn partial_health_weights_inherit_defaults() {
        let config = parse("[checks.codeHealth.weights]\ncomplexity = 12.5\ncrateSize = 0")
            .expect("weights");
        assert_eq!(config.weights.complexity, 1_250);
        assert_eq!(config.weights.size, 2_000);
        assert_eq!(config.weights.nesting, 1_000);
        assert_eq!(config.weights.duplication, 1_500);
        assert_eq!(config.weights.crate_size, 0);
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-HEALTH-007"))]
    fn crate_exceptions_override_only_configured_hard_limits() {
        let config = parse(
            "[checks.codeHealth.crateExceptions.\"packages/demo/Cargo.toml\"]\nmaxCrateSloc = 6000\nmaxCyclomatic = 45\n",
        )
        .expect("crate exceptions");
        assert_eq!(
            config.hard_limits_for_crate("packages/demo/Cargo.toml"),
            super::EffectiveHardLimits {
                max_cyclomatic: 45,
                max_function_tokens: 1_000,
                max_nesting: 8,
                max_crate_sloc: 6_000,
            }
        );
        assert_eq!(
            config.hard_limits_for_crate("packages/other/Cargo.toml"),
            super::EffectiveHardLimits {
                max_cyclomatic: 40,
                max_function_tokens: 1_000,
                max_nesting: 8,
                max_crate_sloc: 5_000,
            }
        );
        for invalid in [
            "[checks.codeHealth.crateExceptions.\"../demo/Cargo.toml\"]\nmaxCrateSloc = 6000\n",
            "[checks.codeHealth.crateExceptions.\"packages/demo/Cargo.toml\"]\nmaxCrateSloc = 1499\n",
            "[checks.codeHealth.crateExceptions.\"packages/demo/Cargo.toml\"]\nmaxCyclomatic = 0\n",
            "[checks.codeHealth.crateExceptions.\"packages/demo/Cargo.toml\"]\nmaxTokens = 1000\n",
        ] {
            assert!(parse(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-044"))]
    fn crate_exceptions_must_match_a_selected_manifest_path() {
        let config = parse(
            "[checks.codeHealth.crateExceptions.\"packages/demo/Cargo.toml\"]\nmaxCrateSloc = 6000\n",
        )
        .expect("crate exception");
        assert!(
            config
                .validate_crate_exception_manifests(&["packages/demo/Cargo.toml".to_owned()])
                .is_ok()
        );
        assert!(
            config
                .validate_crate_exception_manifests(&["packages/other/Cargo.toml".to_owned()])
                .is_err()
        );
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
