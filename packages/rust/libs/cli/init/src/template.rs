include!(concat!(env!("OUT_DIR"), "/templates.rs"));

use super::InitError;

/// Catalog template names in their canonical order.
pub const TEMPLATE_NAMES: &[&str] = &["basic", "axum", "topcoat"];

/// Resolves a requested template name, defaulting to the basic workspace.
pub fn resolve(name: Option<&str>) -> Result<&'static str, InitError> {
    let requested = name.unwrap_or("basic");
    TEMPLATE_NAMES
        .iter()
        .copied()
        .find(|candidate| *candidate == requested)
        .ok_or_else(|| {
            InitError::usage(format!(
                "unknown template '{requested}'; expected one of: {}",
                TEMPLATE_NAMES.join(", ")
            ))
        })
}

/// Iterates the template-relative files of one catalog entry.
pub fn files(template: &'static str) -> impl Iterator<Item = (&'static str, &'static str)> {
    TEMPLATES
        .iter()
        .filter(move |(name, _, _)| *name == template)
        .map(|(_, relative, content)| (*relative, *content))
}
