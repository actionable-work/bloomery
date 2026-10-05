use crate::document;
use crate::error::ConfigError;
use crate::{DiffRecord, DiffStatus};
use bloomery_model::catalog;
use toml_edit::DocumentMut;

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
