use crate::Diagnostic;
use serde::Deserialize;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub specs: SpecsConfig,
    #[serde(default)]
    pub scanners: ScannersConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecsConfig {
    #[serde(default = "default_specs_dir")]
    pub dir: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ScannersConfig {
    #[serde(default)]
    pub rust: RustScannerConfig,
    #[serde(default)]
    pub playwright: PlaywrightScannerConfig,
    #[serde(default)]
    pub nix: NixScannerConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct RustScannerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PlaywrightScannerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default = "default_tag_prefix")]
    pub tag_prefix: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct NixScannerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_checks_attr")]
    pub checks_attr: String,
    #[serde(default)]
    pub systems: Vec<String>,
}

fn default_specs_dir() -> String {
    "specs".to_owned()
}

fn default_tag_prefix() -> String {
    "@bloomery:".to_owned()
}

fn default_checks_attr() -> String {
    ".#checks".to_owned()
}

impl Default for SpecsConfig {
    fn default() -> Self {
        Self {
            dir: default_specs_dir(),
        }
    }
}

#[allow(clippy::result_large_err)]
pub fn load(root: &Path) -> Result<Config, Diagnostic> {
    let path = root.join(".bloomery/config.toml");
    let contents = fs::read_to_string(&path).map_err(|error| {
        Diagnostic::new(
            "ConfigurationError",
            format!("Unable to read configuration: {error}"),
        )
        .at(path.clone(), Some(1))
    })?;
    let config: Config = toml::from_str(&contents).map_err(|error| {
        Diagnostic::new(
            "ConfigurationError",
            format!("Unable to parse configuration: {error}"),
        )
        .at(path, Some(error_span_line(&contents, error.to_string())))
    })?;
    config.validate()?;
    Ok(config)
}

impl Config {
    #[allow(clippy::result_large_err)]
    fn validate(&self) -> Result<(), Diagnostic> {
        if self.specs.dir.trim().is_empty() {
            return Err(Diagnostic::new(
                "ConfigurationError",
                "specs.dir must not be empty",
            ));
        }
        validate_file_scanner(
            "rust",
            self.scanners.rust.enabled,
            &self.scanners.rust.paths,
        )?;
        validate_file_scanner(
            "playwright",
            self.scanners.playwright.enabled,
            &self.scanners.playwright.paths,
        )?;
        if self.scanners.playwright.enabled && self.scanners.playwright.tag_prefix.is_empty() {
            return Err(Diagnostic::new(
                "ConfigurationError",
                "scanners.playwright.tag_prefix must not be empty",
            ));
        }
        if self.scanners.nix.enabled {
            if self.scanners.nix.checks_attr.trim().is_empty() {
                return Err(Diagnostic::new(
                    "ConfigurationError",
                    "scanners.nix.checks_attr must not be empty",
                ));
            }
            if self
                .scanners
                .nix
                .systems
                .iter()
                .any(|system| system.trim().is_empty())
            {
                return Err(Diagnostic::new(
                    "ConfigurationError",
                    "scanners.nix.systems must not contain empty values",
                ));
            }
        }
        Ok(())
    }

    #[allow(clippy::result_large_err)]
    pub fn specs_root(&self, root: &Path) -> Result<PathBuf, Diagnostic> {
        let relative = Path::new(&self.specs.dir);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| component == Component::ParentDir)
        {
            return Err(Diagnostic::new(
                "ConfigurationError",
                "specs.dir must be a relative path inside .bloomery",
            ));
        }
        Ok(root.join(".bloomery").join(relative))
    }
}

#[allow(clippy::result_large_err)]
fn validate_file_scanner(name: &str, enabled: bool, patterns: &[String]) -> Result<(), Diagnostic> {
    if !enabled {
        return Ok(());
    }
    if patterns.is_empty() {
        return Err(Diagnostic::new(
            "ConfigurationError",
            format!("scanners.{name}.paths must contain at least one pattern"),
        ));
    }
    for pattern in patterns {
        let path = Path::new(pattern);
        if pattern.trim().is_empty()
            || path.is_absolute()
            || path
                .components()
                .any(|component| component == Component::ParentDir)
        {
            return Err(Diagnostic::new(
                "ConfigurationError",
                format!("scanners.{name}.paths contains an invalid repository path: {pattern}"),
            ));
        }
    }
    Ok(())
}

fn error_span_line(contents: &str, error: String) -> usize {
    error
        .split(" at line ")
        .nth(1)
        .and_then(|line| line.split_whitespace().next())
        .and_then(|line| line.parse().ok())
        .unwrap_or_else(|| contents.lines().count().max(1))
}
