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

/// Rewrite the version 1 table layout in place, preserving each moved item.
pub(crate) fn apply_migration(
    document: &mut DocumentMut,
    migration: Option<VersionMigration>,
) -> Result<(), ConfigError> {
    let Some(migration) = migration else {
        return Ok(());
    };
    match migration.source_version {
        1 => {
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
        }
        source_version => {
            return Err(ConfigError::failure(format!(
                "no configuration migration is available from version {source_version}"
            )));
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
