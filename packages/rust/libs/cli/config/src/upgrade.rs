use crate::document;
use crate::error::ConfigError;
use crate::{DiffRecord, DiffStatus};
use bloomery_model::catalog;
use bloomery_model::config::CURRENT_CONFIG_VERSION;
use toml_edit::{DocumentMut, Item, Table};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VersionMigration {
    pub(crate) source_version: u32,
    pub(crate) target_version: u32,
}

/// Select any format migrations needed before recommendation changes.
pub(crate) fn migration(
    raw: Option<&toml::Value>,
) -> Result<Option<VersionMigration>, ConfigError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let Some(version) = raw.as_table().and_then(|table| table.get("version")) else {
        return Err(ConfigError::failure(format!(
            "config.toml has no top-level version; set version = {CURRENT_CONFIG_VERSION}"
        )));
    };
    let Some(version) = version.as_integer() else {
        return Err(ConfigError::failure(format!(
            "config.toml version must be integer {CURRENT_CONFIG_VERSION}"
        )));
    };

    match version {
        1 => Ok(Some(VersionMigration {
            source_version: 1,
            target_version: CURRENT_CONFIG_VERSION,
        })),
        2 => Ok(Some(VersionMigration {
            source_version: 2,
            target_version: CURRENT_CONFIG_VERSION,
        })),
        version if version == i64::from(CURRENT_CONFIG_VERSION) => Ok(None),
        version => Err(ConfigError::failure(format!(
            "unsupported config format version {version}; supported version is {CURRENT_CONFIG_VERSION}"
        ))),
    }
}

fn ensure_table<'a>(table: &'a mut Table, key: &str) -> Result<&'a mut Table, ConfigError> {
    if !table.contains_key(key) {
        table.insert(key, Item::Table(Table::new()));
    }
    table
        .get_mut(key)
        .and_then(Item::as_table_mut)
        .ok_or_else(|| ConfigError::failure(format!("{key} is not a configuration table")))
}

fn move_root_table(
    document: &mut DocumentMut,
    source: &str,
    destination: &str,
) -> Result<(), ConfigError> {
    let Some(item) = document.as_table_mut().remove(source) else {
        return Ok(());
    };
    let build = ensure_table(document.as_table_mut(), "build")?;
    if build.contains_key(destination) {
        return Err(ConfigError::failure(format!(
            "cannot migrate [build.{destination}]: the destination already exists"
        )));
    }
    build.insert(destination, item);
    Ok(())
}

fn move_check_value(
    document: &mut DocumentMut,
    source: &str,
    destination: &str,
) -> Result<(), ConfigError> {
    let Some(checks) = document
        .as_table_mut()
        .get_mut("checks")
        .and_then(Item::as_table_mut)
    else {
        return Ok(());
    };
    let Some(value) = checks.remove(source) else {
        return Ok(());
    };
    let nested = ensure_table(checks, destination)?;
    if nested.contains_key("enable") {
        return Err(ConfigError::failure(format!(
            "cannot migrate checks.{source}: checks.{destination}.enable already exists"
        )));
    }
    nested.insert("enable", value);
    Ok(())
}

fn migrate_v1_layout(document: &mut DocumentMut) -> Result<(), ConfigError> {
    move_root_table(document, "toolchain", "toolchain")?;
    move_root_table(document, "profile", "profile")?;
    move_root_table(document, "flags", "flags")?;
    for (source, destination) in [
        ("crateCheck", "crates"),
        ("includePackageChecks", "packages"),
        ("throwOnOutOfDate", "lockFreshness"),
        ("workspaceDependencies", "workspaceDependencies"),
        ("noDefaultFeatures", "noDefaultFeatures"),
    ] {
        move_check_value(document, source, destination)?;
    }
    Ok(())
}

/// A codegen setting that format 3 keeps in a profile instead of a flag list.
#[derive(Debug, Clone, PartialEq)]
struct ProfileSetting {
    field: &'static str,
    value: toml::Value,
}

fn integer_or_string(value: &str) -> toml::Value {
    value.parse::<i64>().map_or_else(
        |_| toml::Value::String(value.to_owned()),
        toml::Value::Integer,
    )
}

/// Parse one rustc codegen option such as `opt-level=3`, `-O`, or `-g`.
fn profile_setting(option: &str) -> Option<ProfileSetting> {
    let setting = |field, value| Some(ProfileSetting { field, value });
    match option {
        "-O" => return setting("optLevel", toml::Value::Integer(2)),
        "-g" => return setting("debuginfo", toml::Value::Integer(2)),
        _ => {}
    }
    let (key, value) = option.split_once('=')?;
    match key {
        "opt-level" => setting("optLevel", integer_or_string(value)),
        "codegen-units" => setting("codegenUnits", integer_or_string(value)),
        "debuginfo" => setting("debuginfo", integer_or_string(value)),
        "overflow-checks" => match value {
            "y" | "yes" | "on" | "true" => setting("overflowChecks", toml::Value::Boolean(true)),
            "n" | "no" | "off" | "false" => setting("overflowChecks", toml::Value::Boolean(false)),
            _ => None,
        },
        _ => None,
    }
}

/// Remove profile-owned codegen flags from `build.flags.<list>`, returning the
/// settings they carried in order.
fn take_profile_settings(document: &mut DocumentMut, list: &str) -> Vec<ProfileSetting> {
    // TableLike lookups never insert placeholder entries for absent keys.
    let Some(array) = document
        .get_mut("build")
        .and_then(Item::as_table_like_mut)
        .and_then(|build| build.get_mut("flags"))
        .and_then(Item::as_table_like_mut)
        .and_then(|flags| flags.get_mut(list))
        .and_then(Item::as_array_mut)
    else {
        return Vec::new();
    };
    let flags = array
        .iter()
        .map(|value| value.as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>();
    let Some(flags) = flags else {
        return Vec::new();
    };

    let mut settings = Vec::new();
    let mut removed = Vec::new();
    let mut index = 0;
    while index < flags.len() {
        let flag = flags[index].as_str();
        let (option, width) = match flag {
            "-C" => match flags.get(index + 1) {
                Some(next) => (next.as_str(), 2),
                None => (flag, 1),
            },
            _ => (
                flag.strip_prefix("-C")
                    .map_or(flag, |rest| rest.trim_start()),
                1,
            ),
        };
        if let Some(setting) = profile_setting(option) {
            settings.push(setting);
            removed.extend(index..index + width);
        }
        index += width;
    }
    for index in removed.into_iter().rev() {
        array.remove(index);
    }
    settings
}

fn profile_has(document: &DocumentMut, profile: &str, field: &str) -> bool {
    document
        .get("build")
        .and_then(|build| build.get("profile"))
        .and_then(|profiles| profiles.get(profile))
        .and_then(|table| table.get(field))
        .is_some()
}

/// Set a profile field unless the configuration already sets it explicitly.
fn set_profile_default(
    document: &mut DocumentMut,
    profile: &str,
    setting: &ProfileSetting,
) -> Result<(), ConfigError> {
    if profile_has(document, profile, setting.field) {
        return Ok(());
    }
    let path = ["build", "profile", profile, setting.field].map(str::to_owned);
    document::set_value(document, &path, &setting.value)?;
    Ok(())
}

/// Format 3 moves codegen settings out of flag lists and into profiles.
/// `build.flags.rustc` fed release crates and binaries; its non-opt-level
/// settings also fed the dev profile. `build.flags.test` fed unit-test
/// harnesses, now compiled with the test profile. Clippy emits no code, so its
/// codegen flags are dropped.
fn migrate_v2_profile_flags(document: &mut DocumentMut) -> Result<(), ConfigError> {
    for setting in take_profile_settings(document, "rustc") {
        let default_opt_level =
            setting.field == "optLevel" && setting.value == toml::Value::Integer(3);
        if !default_opt_level {
            set_profile_default(document, "release", &setting)?;
        }
        if setting.field != "optLevel" {
            set_profile_default(document, "dev", &setting)?;
        }
    }
    for setting in take_profile_settings(document, "test") {
        set_profile_default(document, "test", &setting)?;
    }
    take_profile_settings(document, "clippy");
    Ok(())
}

/// Rewrite older table layouts in place, one format step at a time,
/// preserving each configured value and its meaning.
pub(crate) fn apply_migration(
    document: &mut DocumentMut,
    migration: Option<VersionMigration>,
) -> Result<(), ConfigError> {
    let Some(migration) = migration else {
        return Ok(());
    };
    for version in migration.source_version..migration.target_version {
        match version {
            1 => migrate_v1_layout(document)?,
            2 => migrate_v2_profile_flags(document)?,
            source_version => {
                return Err(ConfigError::failure(format!(
                    "no configuration migration is available from version {source_version}"
                )));
            }
        }
    }
    document::set_value(
        document,
        &["version".to_owned()],
        &toml::Value::Integer(i64::from(migration.target_version)),
    )?;
    Ok(())
}

/// Recommended catalog entries absent from the raw configuration.
pub(crate) fn missing_entries(raw: Option<&toml::Value>) -> Vec<&'static catalog::CatalogEntry> {
    catalog::recommended_entries()
        .filter(|entry| raw.is_none_or(|raw| catalog::raw_at(raw, entry.path).is_none()))
        .collect()
}

/// Add every absent recommended entry, returning the sorted added key paths.
pub(crate) fn apply(
    document: &mut DocumentMut,
    raw: Option<&toml::Value>,
) -> Result<Vec<String>, ConfigError> {
    let mut added = Vec::new();
    for entry in missing_entries(raw) {
        let value = document::recommended_toml(entry).expect("recommended entries have defaults");
        let path = entry
            .path
            .iter()
            .map(|segment| (*segment).to_owned())
            .collect::<Vec<_>>();
        document::set_value(document, &path, &value)?;
        added.push(entry.path.join("."));
    }
    added.sort();
    Ok(added)
}

/// Compare every catalogued key's recommendation with the current configuration.
pub(crate) fn diff(raw: Option<&toml::Value>) -> Vec<DiffRecord> {
    let mut records = catalog::CATALOG
        .iter()
        .map(|entry| {
            let recommended = document::recommended_toml(entry);
            let current = raw
                .and_then(|raw| catalog::raw_at(raw, entry.path))
                .cloned();
            let status = match (&recommended, &current) {
                (None, _) => DiffStatus::Unset,
                (Some(_), None) => DiffStatus::Absent,
                (Some(recommended), Some(current)) if recommended == current => DiffStatus::Equal,
                (Some(_), Some(_)) => DiffStatus::Differing,
            };
            DiffRecord {
                key: entry.path.join("."),
                recommended,
                current,
                status,
            }
        })
        .collect::<Vec<_>>();
    records.sort_by(|left, right| left.key.cmp(&right.key));
    records
}
