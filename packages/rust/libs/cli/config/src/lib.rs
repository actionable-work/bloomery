#![allow(clippy::result_large_err)]

mod document;
mod error;
mod upgrade;

pub use error::{ConfigError, ConfigErrorKind};

use bloomery_cli_types::ConfigOperation;
use bloomery_model::catalog::{self, KeySpec};
use bloomery_model::config::{LoadedConfig, parse_contents};
use serde::Serialize;
use std::fs;
use std::path::Path;
use toml_edit::DocumentMut;

/// Repository-relative location of the configuration file.
pub const CONFIG_RELATIVE_PATH: &str = ".bloomery/config.toml";

/// One catalogued key with its effective value and origin.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct KeyRecord {
    pub key: String,
    pub value: Option<toml::Value>,
    pub configured: bool,
    pub recommended: bool,
}

/// Relationship between a recommended default and the current value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffStatus {
    Equal,
    Differing,
    Absent,
    Unset,
}

/// One recommendation comparison for `upgrade --diff`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiffRecord {
    pub key: String,
    pub recommended: Option<toml::Value>,
    pub current: Option<toml::Value>,
    pub status: DiffStatus,
}

/// Advisory condition reported alongside a successful result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigWarning {
    MissingFlake,
    MissingConfig { path: String },
    CreatedConfig { path: String },
}

/// Command-specific result.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum ConfigOutcome {
    Get {
        key: KeyRecord,
    },
    List {
        keys: Vec<KeyRecord>,
    },
    Set {
        key: String,
        value: toml::Value,
        changed: bool,
    },
    Unset {
        key: String,
        removed: bool,
        changed: bool,
    },
    Upgrade {
        added: Vec<String>,
        written: bool,
        differences: Vec<DiffRecord>,
    },
    Document {
        documented: Vec<String>,
        written: bool,
    },
}

/// Successful result plus any warnings.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConfigReport {
    pub warnings: Vec<ConfigWarning>,
    pub outcome: ConfigOutcome,
}

impl ConfigReport {
    /// Prepend warnings gathered at the CLI boundary.
    pub fn prepend_warnings(&mut self, mut warnings: Vec<ConfigWarning>) {
        warnings.append(&mut self.warnings);
        self.warnings = warnings;
    }
}

enum Resolved {
    Get(KeySpec),
    Set(KeySpec, toml::Value),
    Unset(KeySpec),
    List(Option<String>),
    Upgrade { dry_run: bool, diff: bool },
    Document,
}

impl Resolved {
    fn is_mutating(&self) -> bool {
        match self {
            Self::Set(..) | Self::Unset(..) | Self::Document => true,
            Self::Upgrade { dry_run, diff } => !dry_run && !diff,
            Self::Get(_) | Self::List(_) => false,
        }
    }
}

fn entry_path(entry: &KeySpec) -> Vec<String> {
    entry.path.clone()
}

fn path_refs(path: &[String]) -> Vec<&str> {
    path.iter().map(String::as_str).collect()
}

fn resolve(operation: &ConfigOperation) -> Result<Resolved, ConfigError> {
    Ok(match operation {
        ConfigOperation::Get { key } => {
            let parts = document::parse_key(key)?;
            Resolved::Get(document::lookup_entry(&parts)?)
        }
        ConfigOperation::Set { key, value } => {
            let parts = document::parse_key(key)?;
            let entry = document::lookup_entry(&parts)?;
            let parsed = document::parse_value(&entry, value)?;
            Resolved::Set(entry, parsed)
        }
        ConfigOperation::Unset { key } => {
            let parts = document::parse_key(key)?;
            Resolved::Unset(document::lookup_entry(&parts)?)
        }
        ConfigOperation::List { prefix } => Resolved::List(prefix.clone()),
        ConfigOperation::Upgrade { dry_run, diff } => {
            if *dry_run && *diff {
                return Err(ConfigError::usage(
                    "--dry-run and --diff are mutually exclusive",
                ));
            }
            Resolved::Upgrade {
                dry_run: *dry_run,
                diff: *diff,
            }
        }
        ConfigOperation::Document => Resolved::Document,
    })
}

fn record(entry: &KeySpec, raw: Option<&toml::Value>) -> KeyRecord {
    let path = path_refs(&entry.path);
    KeyRecord {
        key: entry.path.join("."),
        value: catalog::effective_value(raw, &path),
        configured: raw.and_then(|raw| catalog::raw_at(raw, &path)).is_some(),
        recommended: entry.recommended.is_some(),
    }
}

fn publish_checked(config_path: &Path, document: &DocumentMut) -> Result<(), ConfigError> {
    let candidate = document.to_string();
    parse_contents(&candidate, config_path).map_err(ConfigError::from_diagnostic)?;
    document::publish(config_path, &candidate)
}

fn editable_document(contents: Option<&str>) -> Result<DocumentMut, ConfigError> {
    match contents {
        Some(contents) => document::parse_document(contents),
        None => Ok(DocumentMut::new()),
    }
}

/// Execute one config command against the repository rooted at `root`.
pub fn run(root: &Path, operation: &ConfigOperation) -> Result<ConfigReport, ConfigError> {
    let resolved = resolve(operation)?;
    let config_path = root.join(CONFIG_RELATIVE_PATH);
    let mut warnings = Vec::new();

    let exists = config_path.is_file();
    let contents = if exists {
        Some(fs::read_to_string(&config_path).map_err(|error| {
            ConfigError::failure(format!("Unable to read configuration: {error}"))
        })?)
    } else {
        None
    };
    let loaded: Option<LoadedConfig> = match &contents {
        Some(contents) => {
            Some(parse_contents(contents, &config_path).map_err(ConfigError::from_diagnostic)?)
        }
        None => None,
    };
    let raw = loaded.as_ref().and_then(|loaded| loaded.raw.as_ref());

    if !exists {
        if resolved.is_mutating() {
            warnings.push(ConfigWarning::CreatedConfig {
                path: CONFIG_RELATIVE_PATH.to_owned(),
            });
        } else {
            warnings.push(ConfigWarning::MissingConfig {
                path: CONFIG_RELATIVE_PATH.to_owned(),
            });
        }
    }

    let outcome = match &resolved {
        Resolved::Get(entry) => ConfigOutcome::Get {
            key: record(entry, raw),
        },
        Resolved::List(prefix) => {
            let mut keys = catalog::CATALOG
                .iter()
                .filter(|entry| {
                    prefix
                        .as_deref()
                        .is_none_or(|prefix| entry.path.join(".").starts_with(prefix))
                })
                .map(|entry| record(&KeySpec::from_entry(entry), raw))
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.key.cmp(&right.key));
            ConfigOutcome::List { keys }
        }
        Resolved::Set(entry, value) => {
            let mut document = editable_document(contents.as_deref())?;
            let changed = document::set_value(&mut document, &entry_path(entry), value)?;
            let created = !exists;
            if changed || created {
                publish_checked(&config_path, &document)?;
            }
            ConfigOutcome::Set {
                key: entry.path.join("."),
                value: value.clone(),
                changed: changed || created,
            }
        }
        Resolved::Unset(entry) => {
            let mut document = editable_document(contents.as_deref())?;
            let removed = document::unset_value(&mut document, &entry_path(entry))?;
            let created = !exists;
            if removed || created {
                publish_checked(&config_path, &document)?;
            }
            ConfigOutcome::Unset {
                key: entry.path.join("."),
                removed,
                changed: removed || created,
            }
        }
        Resolved::Upgrade { dry_run, diff } => {
            if *diff {
                ConfigOutcome::Upgrade {
                    added: Vec::new(),
                    written: false,
                    differences: upgrade::diff(raw),
                }
            } else {
                let mut added = upgrade::missing_entries(raw)
                    .into_iter()
                    .map(|entry| entry.path.join("."))
                    .collect::<Vec<_>>();
                added.sort();
                let created = !exists;
                let written = if *dry_run || (added.is_empty() && !created) {
                    false
                } else {
                    let mut document = editable_document(contents.as_deref())?;
                    upgrade::apply(&mut document, raw)?;
                    publish_checked(&config_path, &document)?;
                    true
                };
                ConfigOutcome::Upgrade {
                    added,
                    written,
                    differences: Vec::new(),
                }
            }
        }
        Resolved::Document => {
            let mut document = editable_document(contents.as_deref())?;
            let documented = match raw {
                Some(raw) => document::document_values(&mut document, raw)?,
                None => Vec::new(),
            };
            let created = !exists;
            let changed = !documented.is_empty();
            if changed || created {
                publish_checked(&config_path, &document)?;
            }
            ConfigOutcome::Document {
                documented,
                written: changed || created,
            }
        }
    };

    Ok(ConfigReport { warnings, outcome })
}

#[cfg(test)]
mod tests {
    use super::{
        CONFIG_RELATIVE_PATH, ConfigErrorKind, ConfigOutcome, ConfigWarning, DiffStatus, run,
    };
    use bloomery_cli_types::ConfigOperation;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("bloomery-config-command-{suffix}"))
    }

    fn seed(root: &Path, contents: &str) {
        let path = root.join(CONFIG_RELATIVE_PATH);
        fs::create_dir_all(path.parent().expect("config parent")).expect("config dir");
        fs::write(&path, contents).expect("config contents");
    }

    fn contents(root: &Path) -> String {
        fs::read_to_string(root.join(CONFIG_RELATIVE_PATH)).expect("config read")
    }

    fn operation(operation: ConfigOperation) -> ConfigOperation {
        operation
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-COMMANDS-007"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-COMMANDS-012"))]
    fn mutating_command_creates_missing_configuration_with_warning() {
        let root = temp_root();
        let report = run(
            &root,
            &operation(ConfigOperation::Set {
                key: "checks.enable".to_owned(),
                value: "false".to_owned(),
            }),
        )
        .expect("set creates configuration");
        assert!(report.warnings.contains(&ConfigWarning::CreatedConfig {
            path: CONFIG_RELATIVE_PATH.to_owned()
        }));
        assert!(contents(&root).contains("enable = false"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-COMMANDS-008"))]
    fn config_operates_without_a_spec_tree() {
        let root = temp_root();
        let report = run(&root, &operation(ConfigOperation::List { prefix: None }))
            .expect("list without specs");
        assert!(matches!(report.outcome, ConfigOutcome::List { .. }));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-COMMANDS-013"))]
    fn read_only_command_does_not_create_configuration() {
        let root = temp_root();
        let report = run(
            &root,
            &operation(ConfigOperation::Get {
                key: "checks.enable".to_owned(),
            }),
        )
        .expect("get against defaults");
        assert!(report.warnings.contains(&ConfigWarning::MissingConfig {
            path: CONFIG_RELATIVE_PATH.to_owned()
        }));
        assert!(!root.join(CONFIG_RELATIVE_PATH).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-001"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-012"))]
    fn keys_resolve_against_the_schema_including_build_tables() {
        let root = temp_root();
        for key in [
            "checks.enable",
            "profile.release.optLevel",
            "toolchain.rustc",
            "flakes.tests-basic.path",
            "flakes.tests-basic.packages",
            "flakes.tests-basic.apps",
            "flakes.tests-basic.checks",
        ] {
            run(
                &root,
                &operation(ConfigOperation::Get {
                    key: key.to_owned(),
                }),
            )
            .unwrap_or_else(|error| panic!("{key}: {error}"));
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-002"))]
    fn unknown_keys_are_usage_errors() {
        let root = temp_root();
        let error = run(
            &root,
            &operation(ConfigOperation::Get {
                key: "checks.bogus".to_owned(),
            }),
        )
        .expect_err("unknown key");
        assert_eq!(error.kind(), ConfigErrorKind::Usage);
        assert!(!root.join(CONFIG_RELATIVE_PATH).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-003"))]
    fn values_are_parsed_by_schema_type() {
        let root = temp_root();
        run(
            &root,
            &operation(ConfigOperation::Set {
                key: "scanners.rust.paths".to_owned(),
                value: "[\"a/**/*.rs\", \"b/**/*.rs\"]".to_owned(),
            }),
        )
        .expect("list value");
        let report = run(
            &root,
            &operation(ConfigOperation::Get {
                key: "scanners.rust.paths".to_owned(),
            }),
        )
        .expect("get");
        match report.outcome {
            ConfigOutcome::Get { key } => {
                assert_eq!(
                    key.value,
                    Some(toml::Value::Array(vec![
                        toml::Value::String("a/**/*.rs".to_owned()),
                        toml::Value::String("b/**/*.rs".to_owned()),
                    ]))
                );
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-004"))]
    fn type_mismatches_are_usage_errors() {
        let root = temp_root();
        for (key, value) in [
            ("checks.enable", "yes"),
            ("scanners.rust.paths", "not-a-list"),
            ("profile.release.codegenUnits", "0"),
        ] {
            let error = run(
                &root,
                &operation(ConfigOperation::Set {
                    key: key.to_owned(),
                    value: value.to_owned(),
                }),
            )
            .expect_err("type mismatch");
            assert_eq!(error.kind(), ConfigErrorKind::Usage, "{key}");
        }
        assert!(!root.join(CONFIG_RELATIVE_PATH).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-005"))]
    fn get_reports_effective_value_and_origin() {
        let root = temp_root();
        seed(&root, "checks.enable = false\n");
        let report = run(
            &root,
            &operation(ConfigOperation::Get {
                key: "checks.enable".to_owned(),
            }),
        )
        .expect("get");
        match report.outcome {
            ConfigOutcome::Get { key } => {
                assert!(key.configured);
                assert_eq!(key.value, Some(toml::Value::Boolean(false)));
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-006"))]
    fn get_uses_default_for_absent_keys() {
        let root = temp_root();
        seed(&root, "");
        let report = run(
            &root,
            &operation(ConfigOperation::Get {
                key: "checks.enable".to_owned(),
            }),
        )
        .expect("get");
        match report.outcome {
            ConfigOutcome::Get { key } => {
                assert!(!key.configured);
                assert!(key.recommended);
                assert_eq!(key.value, Some(toml::Value::Boolean(true)));
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-007"))]
    fn list_enumerates_the_schema() {
        let root = temp_root();
        seed(&root, "checks.enable = false\n");
        let report = run(&root, &operation(ConfigOperation::List { prefix: None })).expect("list");
        match report.outcome {
            ConfigOutcome::List { keys } => {
                assert!(keys.iter().any(|key| key.key == "specs.dir"));
                assert!(keys.iter().any(|key| key.key == "build.unify"));
                assert!(
                    keys.iter()
                        .all(|key| key.configured == (key.key == "checks.enable"))
                );
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-008"))]
    fn list_filters_by_prefix() {
        let root = temp_root();
        seed(&root, "");
        let report = run(
            &root,
            &operation(ConfigOperation::List {
                prefix: Some("scanners.rust".to_owned()),
            }),
        )
        .expect("list");
        match report.outcome {
            ConfigOutcome::List { keys } => {
                assert!(!keys.is_empty());
                assert!(keys.iter().all(|key| key.key.starts_with("scanners.rust")));
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        let empty = run(
            &root,
            &operation(ConfigOperation::List {
                prefix: Some("does.not.exist".to_owned()),
            }),
        )
        .expect("list");
        assert_eq!(empty.outcome, ConfigOutcome::List { keys: Vec::new() });
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-009"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-010"))]
    fn set_upserts_leaves_and_creates_parents() {
        let root = temp_root();
        seed(&root, "");
        run(
            &root,
            &operation(ConfigOperation::Set {
                key: "checks.enable".to_owned(),
                value: "false".to_owned(),
            }),
        )
        .expect("first set");
        run(
            &root,
            &operation(ConfigOperation::Set {
                key: "checks.enable".to_owned(),
                value: "true".to_owned(),
            }),
        )
        .expect("upsert");
        assert!(contents(&root).contains("enable = true"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-011"))]
    fn mutations_preserve_comments_and_unrelated_formatting() {
        let root = temp_root();
        seed(
            &root,
            "# keep me\n\n[checks]\nenable = true   # trailing\nthrowOnOutOfDate = false\n",
        );
        run(
            &root,
            &operation(ConfigOperation::Set {
                key: "checks.includePackageChecks".to_owned(),
                value: "false".to_owned(),
            }),
        )
        .expect("set");
        let after = contents(&root);
        assert!(after.contains("# keep me"));
        assert!(after.contains("# trailing"));
        assert!(after.contains("throwOnOutOfDate = false"));
        assert!(after.contains("includePackageChecks = false"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-013"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-014"))]
    fn unset_removes_leaves_and_prunes_empty_tables() {
        let root = temp_root();
        seed(&root, "[scanners.rust]\nenabled = false\npaths = [\"a\"]\n");
        run(
            &root,
            &operation(ConfigOperation::Unset {
                key: "scanners.rust.paths".to_owned(),
            }),
        )
        .expect("unset paths");
        run(
            &root,
            &operation(ConfigOperation::Unset {
                key: "scanners.rust.enabled".to_owned(),
            }),
        )
        .expect("unset enabled");
        let after = contents(&root);
        assert!(
            !after.contains("scanners"),
            "emptied table should be pruned: {after}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-015"))]
    fn unset_of_an_absent_key_is_a_no_op() {
        let root = temp_root();
        seed(&root, "checks.enable = true\n");
        let before = contents(&root);
        let report = run(
            &root,
            &operation(ConfigOperation::Unset {
                key: "checks.includePackageChecks".to_owned(),
            }),
        )
        .expect("unset absent");
        match report.outcome {
            ConfigOutcome::Unset {
                removed, changed, ..
            } => {
                assert!(!removed);
                assert!(!changed);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        assert_eq!(contents(&root), before);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-016"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-018"))]
    fn invalid_candidates_are_rejected_without_writing() {
        let root = temp_root();
        seed(&root, "checks.enable = true\n");
        let before = contents(&root);
        let error = run(
            &root,
            &operation(ConfigOperation::Set {
                key: "scanners.rust.enabled".to_owned(),
                value: "true".to_owned(),
            }),
        )
        .expect_err("enabling a scanner without paths is invalid");
        assert_eq!(error.kind(), ConfigErrorKind::Failure);
        assert_eq!(contents(&root), before);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-017"))]
    fn mutations_publish_atomically_without_leftover_temps() {
        let root = temp_root();
        seed(&root, "checks.enable = true\n");
        run(
            &root,
            &operation(ConfigOperation::Set {
                key: "checks.throwOnOutOfDate".to_owned(),
                value: "true".to_owned(),
            }),
        )
        .expect("set");
        let bloomery_dir = root.join(".bloomery");
        let leftovers = fs::read_dir(&bloomery_dir)
            .expect("bloomery dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
            .count();
        assert_eq!(leftovers, 0);
        assert!(contents(&root).contains("throwOnOutOfDate = true"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-019"))]
    fn commands_read_the_current_file_each_invocation() {
        let root = temp_root();
        seed(&root, "checks.enable = true\n");
        let first = run(
            &root,
            &operation(ConfigOperation::Get {
                key: "checks.enable".to_owned(),
            }),
        )
        .expect("first get");
        assert!(matches!(
            first.outcome,
            ConfigOutcome::Get { ref key } if key.value == Some(toml::Value::Boolean(true))
        ));
        seed(&root, "checks.enable = false\n");
        let second = run(
            &root,
            &operation(ConfigOperation::Get {
                key: "checks.enable".to_owned(),
            }),
        )
        .expect("second get");
        assert!(matches!(
            second.outcome,
            ConfigOutcome::Get { ref key } if key.value == Some(toml::Value::Boolean(false))
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-001"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-003"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-004"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-009"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-010"))]
    fn upgrade_adds_absent_recommended_keys_in_sorted_order() {
        let root = temp_root();
        seed(&root, "");
        let report = run(
            &root,
            &operation(ConfigOperation::Upgrade {
                dry_run: false,
                diff: false,
            }),
        )
        .expect("upgrade");
        match report.outcome {
            ConfigOutcome::Upgrade { added, written, .. } => {
                assert!(written);
                assert!(!added.is_empty());
                let mut sorted = added.clone();
                sorted.sort();
                assert_eq!(added, sorted);
                assert!(added.contains(&"checks.enable".to_owned()));
                assert!(added.contains(&"toolchain.rustc".to_owned()));
                assert!(!added.contains(&"toolchain.linker".to_owned()));
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        let after = contents(&root);
        assert!(after.contains("enable = true"));
        assert!(after.contains("throwOnOutOfDate = false"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-002"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-005"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-007"))]
    fn upgrade_preserves_present_values_and_complete_files() {
        let root = temp_root();
        seed(&root, "checks.enable = false\n# note\n");
        let report = run(
            &root,
            &operation(ConfigOperation::Upgrade {
                dry_run: false,
                diff: false,
            }),
        )
        .expect("upgrade");
        match report.outcome {
            ConfigOutcome::Upgrade { added, .. } => {
                assert!(!added.contains(&"checks.enable".to_owned()));
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        let after = contents(&root);
        assert!(after.contains("checks.enable = false"));
        assert!(after.contains("# note"));

        let before_second = contents(&root);
        let second = run(
            &root,
            &operation(ConfigOperation::Upgrade {
                dry_run: false,
                diff: false,
            }),
        )
        .expect("second upgrade");
        match second.outcome {
            ConfigOutcome::Upgrade { added, written, .. } => {
                assert!(added.is_empty());
                assert!(!written);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        assert_eq!(contents(&root), before_second);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-006"))]
    fn upgrade_is_idempotent() {
        let root = temp_root();
        seed(&root, "");
        run(
            &root,
            &operation(ConfigOperation::Upgrade {
                dry_run: false,
                diff: false,
            }),
        )
        .expect("first upgrade");
        let after_first = contents(&root);
        run(
            &root,
            &operation(ConfigOperation::Upgrade {
                dry_run: false,
                diff: false,
            }),
        )
        .expect("second upgrade");
        assert_eq!(contents(&root), after_first);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-008"))]
    fn upgrade_validates_the_upgraded_configuration() {
        let root = temp_root();
        seed(&root, "");
        run(
            &root,
            &operation(ConfigOperation::Upgrade {
                dry_run: false,
                diff: false,
            }),
        )
        .expect("upgrade");
        // The upgraded candidate must itself be a valid configuration.
        let parsed = bloomery_model::config::load(&root).expect("upgraded config validates");
        assert_eq!(parsed.specs.dir, "specs");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-011"))]
    fn dry_run_reports_without_writing() {
        let root = temp_root();
        seed(&root, "checks.enable = true\n");
        let before = contents(&root);
        let report = run(
            &root,
            &operation(ConfigOperation::Upgrade {
                dry_run: true,
                diff: false,
            }),
        )
        .expect("dry run");
        match report.outcome {
            ConfigOutcome::Upgrade { added, written, .. } => {
                assert!(!added.is_empty());
                assert!(!written);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        assert_eq!(contents(&root), before);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-012"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-013"))]
    fn upgrade_materializes_build_keys_but_not_optional_keys() {
        let root = temp_root();
        seed(&root, "");
        run(
            &root,
            &operation(ConfigOperation::Upgrade {
                dry_run: false,
                diff: false,
            }),
        )
        .expect("upgrade");
        let after = contents(&root);
        assert!(after.contains("rustc = \"rustc\""));
        assert!(after.contains("profileName = \"release\""));
        assert!(!after.contains("linker ="));
        assert!(!after.contains("bloomeryLock"));
        assert!(!after.contains("members ="));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-014"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-015"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-016"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-017"))]
    fn diff_compares_recommendations_without_writing() {
        let root = temp_root();
        seed(&root, "checks.enable = false\n");
        let before = contents(&root);
        let report = run(
            &root,
            &operation(ConfigOperation::Upgrade {
                dry_run: false,
                diff: true,
            }),
        )
        .expect("diff");
        match report.outcome {
            ConfigOutcome::Upgrade {
                differences,
                written,
                added,
            } => {
                assert!(!written);
                assert!(added.is_empty());
                let enable = differences
                    .iter()
                    .find(|record| record.key == "checks.enable")
                    .expect("checks.enable diff");
                assert_eq!(enable.status, DiffStatus::Differing);
                assert_eq!(enable.current, Some(toml::Value::Boolean(false)));
                assert_eq!(enable.recommended, Some(toml::Value::Boolean(true)));
                let absent = differences
                    .iter()
                    .find(|record| record.key == "build.unify")
                    .expect("build.unify diff");
                assert_eq!(absent.status, DiffStatus::Absent);
                let optional = differences
                    .iter()
                    .find(|record| record.key == "toolchain.linker")
                    .expect("linker diff");
                assert_eq!(optional.status, DiffStatus::Unset);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        assert_eq!(contents(&root), before);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-018"))]
    fn diff_and_dry_run_are_mutually_exclusive() {
        let root = temp_root();
        let error = run(
            &root,
            &operation(ConfigOperation::Upgrade {
                dry_run: true,
                diff: true,
            }),
        )
        .expect_err("ambiguous flags");
        assert_eq!(error.kind(), ConfigErrorKind::Usage);
        assert!(!root.join(CONFIG_RELATIVE_PATH).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-020"))]
    fn upgrade_materializes_a_missing_configuration() {
        let root = temp_root();
        let report = run(
            &root,
            &operation(ConfigOperation::Upgrade {
                dry_run: false,
                diff: false,
            }),
        )
        .expect("upgrade");
        assert!(report.warnings.contains(&ConfigWarning::CreatedConfig {
            path: CONFIG_RELATIVE_PATH.to_owned()
        }));
        assert!(contents(&root).contains("enable = true"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-DOCUMENT-001"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-DOCUMENT-002"))]
    fn document_writes_catalog_documentation_inline() {
        let root = temp_root();
        seed(&root, "[checks]\nenable = true\n");
        let report = run(&root, &operation(ConfigOperation::Document)).expect("document");
        match report.outcome {
            ConfigOutcome::Document {
                documented,
                written,
            } => {
                assert!(written);
                assert_eq!(documented, vec!["checks.enable".to_owned()]);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        let after = contents(&root);
        assert!(
            after.contains("enable = true # Generate workspace checks."),
            "{after}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-DOCUMENT-003"))]
    fn document_leaves_absent_keys_absent() {
        let root = temp_root();
        seed(&root, "[checks]\nenable = true\n");
        run(&root, &operation(ConfigOperation::Document)).expect("document");
        let after = contents(&root);
        assert!(!after.contains("includePackageChecks"));
        assert!(!after.contains("throwOnOutOfDate"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-DOCUMENT-004"))]
    fn document_preserves_configured_values() {
        let root = temp_root();
        seed(&root, "[checks]\nenable = false\n");
        run(&root, &operation(ConfigOperation::Document)).expect("document");
        assert!(contents(&root).contains("enable = false"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-DOCUMENT-005"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-DOCUMENT-012"))]
    fn document_preserves_trailing_comments_with_fallback() {
        let root = temp_root();
        seed(&root, "# keep me\n[checks]\nenable = true # my note\n");
        run(&root, &operation(ConfigOperation::Document)).expect("document");
        let after = contents(&root);
        assert!(after.contains("# keep me"), "{after}");
        assert!(after.contains("enable = true # my note"), "{after}");
        assert!(
            after.contains("# Generate workspace checks.\nenable = true"),
            "{after}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-DOCUMENT-006"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-DOCUMENT-011"))]
    fn document_is_idempotent() {
        let root = temp_root();
        seed(&root, "[checks]\nenable = true\n");
        run(&root, &operation(ConfigOperation::Document)).expect("first document");
        let after_first = contents(&root);
        let report = run(&root, &operation(ConfigOperation::Document)).expect("second document");
        match report.outcome {
            ConfigOutcome::Document {
                documented,
                written,
            } => {
                assert!(documented.is_empty());
                assert!(!written);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        assert_eq!(contents(&root), after_first);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-DOCUMENT-007"))]
    fn document_creates_missing_configuration() {
        let root = temp_root();
        let report = run(&root, &operation(ConfigOperation::Document)).expect("document");
        assert!(report.warnings.contains(&ConfigWarning::CreatedConfig {
            path: CONFIG_RELATIVE_PATH.to_owned()
        }));
        match report.outcome {
            ConfigOutcome::Document {
                documented,
                written,
            } => {
                assert!(documented.is_empty());
                assert!(written);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        assert!(root.join(CONFIG_RELATIVE_PATH).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-DOCUMENT-008"))]
    fn document_rejects_invalid_configuration() {
        let root = temp_root();
        seed(&root, "[checks]\nenable = \"yes\"\n");
        let before = contents(&root);
        let error = run(&root, &operation(ConfigOperation::Document)).expect_err("invalid config");
        assert_eq!(error.kind(), ConfigErrorKind::Failure);
        assert_eq!(contents(&root), before);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-DOCUMENT-009"))]
    fn document_publishes_atomically() {
        let root = temp_root();
        seed(&root, "[checks]\nenable = true\n");
        run(&root, &operation(ConfigOperation::Document)).expect("document");
        let bloomery_dir = root.join(".bloomery");
        let leftovers = fs::read_dir(&bloomery_dir)
            .expect("bloomery dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
            .count();
        assert_eq!(leftovers, 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-DOCUMENT-010"))]
    fn document_reports_sorted_paths() {
        let root = temp_root();
        seed(&root, "[checks]\nenable = true\nthrowOnOutOfDate = false\n");
        let report = run(&root, &operation(ConfigOperation::Document)).expect("document");
        match report.outcome {
            ConfigOutcome::Document { documented, .. } => {
                let mut sorted = documented.clone();
                sorted.sort();
                assert_eq!(documented, sorted);
                assert!(documented.contains(&"checks.enable".to_owned()));
                assert!(documented.contains(&"checks.throwOnOutOfDate".to_owned()));
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-DOCUMENT-001"))]
    fn document_covers_extra_formatter_names() {
        let root = temp_root();
        seed(&root, "[formatters.prettier]\nenable = true\n");
        let report = run(&root, &operation(ConfigOperation::Document)).expect("document");
        match report.outcome {
            ConfigOutcome::Document { documented, .. } => {
                assert!(documented.contains(&"formatters.prettier.enable".to_owned()));
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        let after = contents(&root);
        assert!(
            after.contains("Enable or disable this formatter."),
            "{after}"
        );
        let _ = fs::remove_dir_all(root);
    }
}
