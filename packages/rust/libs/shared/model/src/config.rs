use crate::Diagnostic;
use serde::Deserialize;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub specs: SpecsConfig,
    #[serde(default)]
    pub scanners: ScannersConfig,
    // Nix build tables. They are owned by the flake interface and kept opaque
    // to the CLI so a shared config file parses without interpreting them.
    #[serde(default)]
    pub build: Option<toml::Value>,
    #[serde(default)]
    pub toolchain: Option<toml::Value>,
    #[serde(default)]
    pub profile: Option<toml::Value>,
    #[serde(default)]
    pub flags: Option<toml::Value>,
    #[serde(default, rename = "devShell")]
    pub dev_shell: Option<toml::Value>,
    #[serde(default)]
    pub checks: Option<toml::Value>,
    #[serde(default)]
    pub features: Option<toml::Value>,
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

#[derive(Debug, Clone, Deserialize)]
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
    #[serde(default)]
    pub paths: Vec<String>,
}

fn default_specs_dir() -> String {
    "specs".to_owned()
}

fn default_tag_prefix() -> String {
    "@bloomery:".to_owned()
}

impl Default for SpecsConfig {
    fn default() -> Self {
        Self {
            dir: default_specs_dir(),
        }
    }
}

impl Default for PlaywrightScannerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            paths: Vec::new(),
            tag_prefix: default_tag_prefix(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub config: Config,
    /// The user-authored TOML tree, before typed defaults are inserted.
    /// `None` means that the configuration file does not exist.
    pub raw: Option<toml::Value>,
}

#[allow(clippy::result_large_err)]
pub fn load(root: &Path) -> Result<Config, Diagnostic> {
    load_with_raw(root).map(|loaded| loaded.config)
}

#[allow(clippy::result_large_err)]
pub fn load_with_raw(root: &Path) -> Result<LoadedConfig, Diagnostic> {
    let path = root.join(".bloomery/config.toml");
    let (config, raw) = match fs::read_to_string(&path) {
        Ok(contents) => {
            let raw = toml::from_str::<toml::Value>(&contents).map_err(|error| {
                Diagnostic::new(
                    "ConfigurationError",
                    format!("Unable to parse configuration: {error}"),
                )
                .at(
                    path.clone(),
                    Some(error_span_line(&contents, error.to_string())),
                )
            })?;
            let config: Config = toml::from_str(&contents).map_err(|error| {
                Diagnostic::new(
                    "ConfigurationError",
                    format!("Unable to parse configuration: {error}"),
                )
                .at(
                    path.clone(),
                    Some(error_span_line(&contents, error.to_string())),
                )
            })?;
            (config, Some(raw))
        }
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && fs::symlink_metadata(&path).is_err_and(|metadata_error| {
                    metadata_error.kind() == std::io::ErrorKind::NotFound
                }) =>
        {
            return Err(Diagnostic::new(
                "ConfigurationError",
                "config.toml is required; create .bloomery/config.toml to configure Bloomery",
            )
            .at(path, Some(1)));
        }
        Err(error) => {
            return Err(Diagnostic::new(
                "ConfigurationError",
                format!("Unable to read configuration: {error}"),
            )
            .at(path, Some(1)));
        }
    };
    config.validate()?;
    Ok(LoadedConfig { config, raw })
}

impl Config {
    #[allow(clippy::result_large_err)]
    fn validate(&self) -> Result<(), Diagnostic> {
        self.validate_specs_dir()?;
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
        validate_file_scanner("nix", self.scanners.nix.enabled, &self.scanners.nix.paths)?;
        Ok(())
    }

    #[allow(clippy::result_large_err)]
    fn validate_specs_dir(&self) -> Result<(), Diagnostic> {
        if self.specs.dir.trim().is_empty() {
            return Err(Diagnostic::new(
                "ConfigurationError",
                "specs.dir must not be empty",
            ));
        }
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
        Ok(())
    }

    #[allow(clippy::result_large_err)]
    pub fn specs_root(&self, root: &Path) -> Result<PathBuf, Diagnostic> {
        self.validate_specs_dir()?;
        Ok(root.join(".bloomery").join(&self.specs.dir))
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

#[cfg(test)]
mod tests {
    use super::{NixScannerConfig, PlaywrightScannerConfig, RustScannerConfig, load};
    use bloomery_test_macros::bloomery;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("bloomery-config-{suffix}"))
    }

    fn assert_default_scanners(
        rust: &RustScannerConfig,
        playwright: &PlaywrightScannerConfig,
        nix: &NixScannerConfig,
    ) {
        assert!(!rust.enabled);
        assert!(rust.paths.is_empty());
        assert!(!playwright.enabled);
        assert!(playwright.paths.is_empty());
        assert_eq!(playwright.tag_prefix, "@bloomery:");
        assert!(!nix.enabled);
        assert!(nix.paths.is_empty());
    }

    fn assert_configuration_error(root: &Path) {
        let diagnostic = load(root).expect_err("configuration should be rejected");
        assert_eq!(diagnostic.code, "ConfigurationError");
    }

    #[test]
    #[bloomery("PARSER-CONFIGURATION-REQUIRED-001")]
    fn missing_configuration_is_an_error() {
        let root = root();
        assert_configuration_error(&root);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[bloomery("PARSER-CONFIGURATION-REQUIRED-002")]
    fn partial_configuration_uses_defaults_for_omitted_tables_and_keys() {
        let root = root();
        let config_path = root.join(".bloomery/config.toml");
        fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");

        fs::write(&config_path, "").expect("empty config");
        let config = load(&root).expect("empty config should use defaults");
        assert_eq!(config.specs.dir, "specs");
        assert_default_scanners(
            &config.scanners.rust,
            &config.scanners.playwright,
            &config.scanners.nix,
        );

        fs::write(&config_path, "[specs]\n").expect("partial config");
        let config = load(&root).expect("omitted scanner table should default");
        assert_eq!(config.specs.dir, "specs");
        assert_default_scanners(
            &config.scanners.rust,
            &config.scanners.playwright,
            &config.scanners.nix,
        );

        fs::write(
            &config_path,
            "[scanners.rust]\nenabled = false\n\
             [scanners.playwright]\nenabled = false\n\
             [scanners.nix]\nenabled = false\n",
        )
        .expect("partial scanner config");
        let config = load(&root).expect("omitted scanner keys should default");
        assert_eq!(config.specs.dir, "specs");
        assert_default_scanners(
            &config.scanners.rust,
            &config.scanners.playwright,
            &config.scanners.nix,
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[bloomery("PARSER-CONFIGURATION-REQUIRED-003")]
    #[bloomery("PARSER-CONFIGURATION-SECTIONS-001")]
    fn build_tables_are_accepted_as_valid_configuration() {
        let root = root();
        let config_path = root.join(".bloomery/config.toml");
        fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");

        fs::write(
            &config_path,
            "[build]\ncargoToml = \"Cargo.toml\"\nlibPackages = true\n\
             [toolchain]\nlinker = \"lld\"\n\
             [profile.release]\noptLevel = 3\n\
             [profile.dev]\noptLevel = 0\n\
             [flags]\ndoc = [\"-Dwarnings\"]\n\
             [devShell]\nenable = false\n\
             [checks]\nenable = true\n\
             [features]\nunify = true\n",
        )
        .expect("build tables");
        let config = load(&root).expect("build tables should be accepted");
        assert_eq!(config.specs.dir, "specs");
        assert_default_scanners(
            &config.scanners.rust,
            &config.scanners.playwright,
            &config.scanners.nix,
        );
        assert!(config.build.is_some());
        assert!(config.toolchain.is_some());
        assert!(config.profile.is_some());
        assert!(config.flags.is_some());
        assert!(config.dev_shell.is_some());
        assert!(config.checks.is_some());
        assert!(config.features.is_some());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[bloomery("PARSER-CONFIGURATION-REQUIRED-004")]
    fn present_malformed_unreadable_or_invalid_configuration_is_an_error() {
        let root = root();
        let config_path = root.join(".bloomery/config.toml");
        fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");

        fs::write(&config_path, "[specs\n").expect("malformed config");
        assert_configuration_error(&root);

        fs::remove_file(&config_path).expect("remove malformed config");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("missing-target", &config_path).expect("dangling config");
            assert_configuration_error(&root);
            fs::remove_file(&config_path).expect("remove dangling config");
        }
        fs::create_dir(&config_path).expect("unreadable config directory");
        assert_configuration_error(&root);

        fs::remove_dir(&config_path).expect("remove config directory");
        fs::write(
            &config_path,
            "[scanners.rust]\nenabled = true\npaths = []\n",
        )
        .expect("invalid Rust scanner config");
        assert_configuration_error(&root);

        fs::write(&config_path, "[scanners.nix]\nenabled = true\npaths = []\n")
            .expect("Nix scanner without paths");
        assert_configuration_error(&root);

        fs::write(
            &config_path,
            "[scanners.nix]\nenabled = true\npaths = [\"../checks.nix\"]\n",
        )
        .expect("Nix scanner with an escaping path");
        assert_configuration_error(&root);
        let _ = fs::remove_dir_all(root);
    }
}
