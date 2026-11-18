//! Shared readers for CLI-owned `[checks.*]` policy tables.

use crate::Diagnostic;
use toml::Value;

/// Enablement and string list of one `[checks.<name>]` policy table.
///
/// An absent table is disabled with an empty list. Value types are validated
/// by the schema catalog before policies are read.
pub(crate) fn enabled_string_list(
    raw: Option<&Value>,
    name: &str,
    list_key: &str,
) -> Result<(bool, Vec<String>), Diagnostic> {
    let Some(table) = raw
        .and_then(Value::as_table)
        .and_then(|root| root.get("checks"))
        .and_then(Value::as_table)
        .and_then(|checks| checks.get(name))
    else {
        return Ok((false, Vec::new()));
    };
    let table = table.as_table().ok_or_else(|| {
        Diagnostic::new(
            "ConfigurationError",
            format!("checks.{name} must be a table"),
        )
    })?;
    let enabled = table
        .get("enable")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let values = table
        .get(list_key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Ok((enabled, values))
}
