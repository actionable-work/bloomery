use crate::error::ConfigError;
use bloomery_model::catalog::{self, CatalogEntry, DefaultValue, KeySpec, ValueType};
use std::fs;
use std::path::Path;
use toml_edit::{Array, DocumentMut, Item, RawString, Table, Value as EditValue};

/// Split a dotted key path, rejecting empty or malformed segments.
pub(crate) fn parse_key(key: &str) -> Result<Vec<String>, ConfigError> {
    let segments = key.split('.').map(str::to_owned).collect::<Vec<String>>();
    let valid = !segments.is_empty()
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment.chars().all(|character| {
                    character.is_ascii_alphanumeric() || character == '_' || character == '-'
                })
        });
    if !valid {
        return Err(ConfigError::usage(format!(
            "invalid configuration key: {key}"
        )));
    }
    Ok(segments)
}

/// Resolve a parsed key path to a catalog entry or parameterized key.
pub(crate) fn lookup_entry(parts: &[String]) -> Result<KeySpec, ConfigError> {
    let reference = parts.iter().map(String::as_str).collect::<Vec<_>>();
    catalog::resolve(&reference).ok_or_else(|| {
        ConfigError::usage(format!("unknown configuration key: {}", parts.join(".")))
    })
}

fn parse_bool(raw: &str) -> Option<bool> {
    match raw {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn parse_toml_expression(raw: &str) -> Option<toml::Value> {
    let document = format!("value = {raw}");
    let mut parsed = toml::from_str::<toml::Value>(&document).ok()?;
    parsed.as_table_mut()?.remove("value")
}

fn parse_typed(value_type: &ValueType, raw: &str) -> Option<toml::Value> {
    match value_type {
        ValueType::Bool => parse_bool(raw).map(toml::Value::Boolean),
        ValueType::Integer => raw.parse::<i64>().ok().map(toml::Value::Integer),
        ValueType::PositiveInteger => raw
            .parse::<i64>()
            .ok()
            .filter(|value| *value > 0)
            .map(toml::Value::Integer),
        ValueType::UnsignedUpTo(max) => raw
            .parse::<i64>()
            .ok()
            .filter(|value| *value >= 0 && *value as u64 <= *max)
            .map(toml::Value::Integer),
        ValueType::String | ValueType::Path | ValueType::Package => {
            Some(toml::Value::String(raw.to_owned()))
        }
        ValueType::StringList => {
            let parsed = parse_toml_expression(raw)?;
            parsed
                .as_array()
                .is_some_and(|items| items.iter().all(toml::Value::is_str))
                .then_some(parsed)
        }
        ValueType::StringEnum(values) => values
            .contains(&raw)
            .then(|| toml::Value::String(raw.to_owned())),
        ValueType::Union(types) => types
            .iter()
            .find_map(|value_type| parse_typed(value_type, raw)),
    }
}

/// Parse a supplied value according to a catalog type.
pub(crate) fn parse_value(entry: &KeySpec, raw: &str) -> Result<toml::Value, ConfigError> {
    parse_typed(&entry.value_type, raw).ok_or_else(|| {
        ConfigError::usage(format!(
            "{} must be a {}",
            entry.path.join("."),
            catalog::describe_value_type(&entry.value_type)
        ))
    })
}

/// Render the recommended default for an entry.
pub(crate) fn recommended_toml(entry: &CatalogEntry) -> Option<toml::Value> {
    entry.recommended.map(DefaultValue::to_toml)
}

fn to_edit(value: &toml::Value) -> Result<EditValue, ConfigError> {
    Ok(match value {
        toml::Value::Boolean(value) => EditValue::from(*value),
        toml::Value::Integer(value) => EditValue::from(*value),
        toml::Value::String(value) => EditValue::from(value.clone()),
        toml::Value::Array(items) => {
            let mut array = Array::new();
            for item in items {
                array.push(to_edit(item)?);
            }
            EditValue::Array(array)
        }
        other => {
            return Err(ConfigError::failure(format!(
                "unsupported configuration value: {other}"
            )));
        }
    })
}

fn navigate_mut<'a>(
    table: &'a mut Table,
    parents: &[String],
) -> Result<&'a mut Table, ConfigError> {
    let mut current = table;
    for segment in parents {
        if !current.contains_key(segment) {
            current.insert(segment, Item::Table(Table::new()));
        }
        current = current
            .get_mut(segment)
            .and_then(Item::as_table_mut)
            .ok_or_else(|| {
                ConfigError::failure(format!("{segment} is not a configuration table"))
            })?;
    }
    Ok(current)
}

fn navigate_existing_mut<'a>(table: &'a mut Table, parents: &[String]) -> Option<&'a mut Table> {
    let mut current = table;
    for segment in parents {
        current = current.get_mut(segment).and_then(Item::as_table_mut)?;
    }
    Some(current)
}

fn prune_empty(table: &mut Table, parents: &[String]) {
    if let Some((first, rest)) = parents.split_first() {
        let Some(child) = table.get_mut(first).and_then(Item::as_table_mut) else {
            return;
        };
        prune_empty(child, rest);
        if child.is_empty() {
            table.remove(first);
        }
    }
}

fn edit_matches(existing: &EditValue, desired: &toml::Value) -> bool {
    match (existing, desired) {
        (EditValue::Boolean(_), toml::Value::Boolean(want)) => existing.as_bool() == Some(*want),
        (EditValue::Integer(_), toml::Value::Integer(want)) => existing.as_integer() == Some(*want),
        (EditValue::String(_), toml::Value::String(want)) => existing.as_str() == Some(want),
        (EditValue::Array(items), toml::Value::Array(want)) => {
            items.len() == want.len()
                && items
                    .iter()
                    .zip(want.iter())
                    .all(|(item, want)| edit_matches(item, want))
        }
        _ => false,
    }
}

/// Upsert a value, returning whether the document changed.
pub(crate) fn set_value(
    document: &mut DocumentMut,
    path: &[String],
    value: &toml::Value,
) -> Result<bool, ConfigError> {
    let edit = to_edit(value)?;
    let (last, parents) = path
        .split_last()
        .expect("catalogued key paths are non-empty");
    let table = navigate_mut(document.as_table_mut(), parents)?;
    let changed = table
        .get(last)
        .and_then(Item::as_value)
        .is_none_or(|existing| !edit_matches(existing, value));
    if changed {
        table.insert(last, Item::Value(edit));
    }
    Ok(changed)
}

/// Remove a leaf, pruning any parent tables it emptied.
pub(crate) fn unset_value(
    document: &mut DocumentMut,
    path: &[String],
) -> Result<bool, ConfigError> {
    let (last, parents) = path
        .split_last()
        .expect("catalogued key paths are non-empty");
    let Some(table) = navigate_existing_mut(document.as_table_mut(), parents) else {
        return Ok(false);
    };
    let removed = table.remove(last).is_some();
    if removed {
        prune_empty(document.as_table_mut(), parents);
    }
    Ok(removed)
}

/// Parse TOML into an editable document.
pub(crate) fn parse_document(contents: &str) -> Result<DocumentMut, ConfigError> {
    contents
        .parse::<DocumentMut>()
        .map_err(|error| ConfigError::failure(format!("Unable to parse configuration: {error}")))
}

fn documentation_comment(documentation: &str) -> String {
    format!("# {documentation}")
}

fn has_documentation_line(text: &str, comment: &str) -> bool {
    text.lines().any(|line| line.trim() == comment)
}

/// Annotate one key with its catalog documentation. Returns whether the
/// document changed. The documentation is written as a trailing comment when
/// the value has no trailing comment, and above the key otherwise.
fn document_key(table: &mut Table, key: &str, documentation: &str) -> bool {
    let Some((mut key_mut, item)) = table.get_key_value_mut(key) else {
        return false;
    };
    let comment = documentation_comment(documentation);
    if let Some(value) = item.as_value_mut() {
        let suffix = value
            .decor()
            .suffix()
            .and_then(RawString::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned();
        if suffix == comment {
            return false;
        }
        if suffix.is_empty() {
            value.decor_mut().set_suffix(format!(" {comment}"));
            return true;
        }
    }
    let prefix = key_mut
        .leaf_decor()
        .prefix()
        .and_then(RawString::as_str)
        .unwrap_or_default()
        .to_owned();
    if has_documentation_line(&prefix, &comment) {
        return false;
    }
    key_mut
        .leaf_decor_mut()
        .set_prefix(format!("{prefix}{comment}\n"));
    true
}

fn document_leaf(document: &mut DocumentMut, path: &[String], documentation: &str) -> bool {
    let Some((last, parents)) = path.split_last() else {
        return false;
    };
    let Some(table) = navigate_existing_mut(document.as_table_mut(), parents) else {
        return false;
    };
    document_key(table, last, documentation)
}

fn formatter_extra_documentation(
    document: &mut DocumentMut,
    raw: &toml::Value,
    documented: &mut Vec<String>,
) {
    let Some(formatters) = raw.get("formatters").and_then(toml::Value::as_table) else {
        return;
    };
    for name in formatters.keys() {
        if catalog::BUILTIN_FORMATTERS.contains(&name.as_str()) {
            continue;
        }
        for field in ["enable", "before", "after"] {
            let path = vec!["formatters".to_owned(), name.clone(), field.to_owned()];
            let reference = path.iter().map(String::as_str).collect::<Vec<_>>();
            let Some(spec) = catalog::resolve(&reference) else {
                continue;
            };
            if document_leaf(document, &path, spec.documentation) {
                documented.push(path.join("."));
            }
        }
    }
}

/// Annotate every catalogued key present in `raw` with its documentation.
/// Returns the sorted paths that gained a documentation comment.
pub(crate) fn document_values(
    document: &mut DocumentMut,
    raw: &toml::Value,
) -> Result<Vec<String>, ConfigError> {
    let mut documented = Vec::new();
    for entry in catalog::CATALOG {
        let path = entry
            .path
            .iter()
            .map(|segment| (*segment).to_owned())
            .collect::<Vec<_>>();
        if document_leaf(document, &path, entry.documentation) {
            documented.push(entry.path.join("."));
        }
    }
    formatter_extra_documentation(document, raw, &mut documented);
    documented.sort();
    Ok(documented)
}

/// Write contents beside the target and atomically rename into place.
pub(crate) fn publish(path: &Path, contents: &str) -> Result<(), ConfigError> {
    let directory = path
        .parent()
        .ok_or_else(|| ConfigError::failure("configuration path has no parent directory"))?;
    fs::create_dir_all(directory).map_err(|error| {
        ConfigError::failure(format!("Unable to create the .bloomery directory: {error}"))
    })?;
    let temporary = directory.join(format!(".config.toml.{}.tmp", std::process::id()));
    fs::write(&temporary, contents).map_err(|error| {
        ConfigError::failure(format!(
            "Unable to write the temporary configuration: {error}"
        ))
    })?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        ConfigError::failure(format!("Unable to replace the configuration file: {error}"))
    })
}
