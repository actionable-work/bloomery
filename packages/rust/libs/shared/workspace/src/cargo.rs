//! Static projection of explicitly declared Cargo integration-test entrypoints.

use crate::source::{SourceTree, TreeIndex, validate_path};
use bloomery_model::Diagnostic;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestTarget {
    pub manifest: String,
    pub name: String,
    pub path: String,
    pub required_features: BTreeSet<String>,
    pub harness: bool,
}

/// Validate a selector without reading content or consulting Cargo.
pub fn validate_selector(manifest: &str, name: &str) -> Result<(), Diagnostic> {
    validate_path(manifest)
        .map_err(|error| finding(manifest, "CargoTargetSelectorError", error))?;
    if manifest.rsplit('/').next() != Some("Cargo.toml") || name.trim().is_empty() {
        return Err(finding(
            manifest,
            "CargoTargetSelectorError",
            "expected a Cargo.toml path and nonempty test target name",
        ));
    }
    Ok(())
}

/// Read one declared target. This is neither Cargo validation nor auto-discovery.
pub fn load_test_target(
    tree: &dyn SourceTree,
    manifest: &str,
    name: &str,
) -> Result<TestTarget, Diagnostic> {
    validate_selector(manifest, name)?;
    let index =
        TreeIndex::new(tree).map_err(|error| finding(manifest, "SourceTreeError", error))?;
    if !index.is_file(std::path::Path::new(manifest)) {
        return Err(finding(
            manifest,
            "CargoTargetReadError",
            "manifest is not an indexed file",
        ));
    }
    let source = tree.read(manifest).map_err(|error| {
        finding(
            manifest,
            "CargoTargetReadError",
            format!("unable to read manifest: {error}"),
        )
    })?;
    let document: toml::Value = toml::from_str(&source)
        .map_err(|error| finding(manifest, "CargoTargetManifestError", error.to_string()))?;
    let declarations = document
        .get("test")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| {
            finding(
                manifest,
                "CargoTestTargetNotFound",
                "no explicit [[test]] declarations",
            )
        })?;
    let mut selected = None;
    for declaration in declarations {
        let declared_name = declaration
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| {
                finding(
                    manifest,
                    "CargoTargetManifestError",
                    "each explicit test declaration needs a string name",
                )
            })?;
        if declared_name == name && selected.replace(declaration).is_some() {
            return Err(finding(
                manifest,
                "DuplicateCargoTestTarget",
                format!("multiple test declarations named {name}"),
            ));
        }
    }
    let declaration = selected.ok_or_else(|| {
        finding(
            manifest,
            "CargoTestTargetNotFound",
            format!("no explicit test target named {name}"),
        )
    })?;
    let raw_path = declaration
        .get("path")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            finding(
                manifest,
                "UnsupportedCargoTestTarget",
                "selected test target needs an explicit string path",
            )
        })?;
    let path = entrypoint_path(manifest, raw_path)
        .map_err(|error| finding(manifest, "CargoTargetPathError", error))?;
    if !index.is_file(std::path::Path::new(&path)) {
        return Err(finding(
            manifest,
            "CargoTargetPathError",
            format!("entrypoint is not an indexed file: {path}"),
        ));
    }
    // Reading the indexed entrypoint also rejects unavailable virtual content.
    tree.read_bytes(&path).map_err(|error| {
        finding(
            manifest,
            "CargoTargetReadError",
            format!("unable to read entrypoint {path}: {error}"),
        )
    })?;
    let harness = match declaration.get("harness") {
        None => true,
        Some(value) => value.as_bool().ok_or_else(|| {
            finding(
                manifest,
                "CargoTargetManifestError",
                "harness must be Boolean",
            )
        })?,
    };
    let required_features = required_features(declaration)
        .map_err(|error| finding(manifest, "CargoTargetManifestError", error))?;
    Ok(TestTarget {
        manifest: manifest.into(),
        name: name.into(),
        path,
        required_features,
        harness,
    })
}

fn required_features(declaration: &toml::Value) -> Result<BTreeSet<String>, String> {
    let Some(value) = declaration.get("required-features") else {
        return Ok(BTreeSet::new());
    };
    value
        .as_array()
        .ok_or("required-features must be an array")?
        .iter()
        .map(|feature| {
            feature
                .as_str()
                .filter(|feature| !feature.trim().is_empty())
                .map(String::from)
                .ok_or_else(|| "required-features entries must be nonempty strings".into())
        })
        .collect()
}

fn entrypoint_path(manifest: &str, path: &str) -> Result<String, String> {
    if path.is_empty() || path.starts_with('/') || path.contains(['\\', ':', '\0']) {
        return Err("entrypoint must be a relative portable path".into());
    }
    let mut parts: Vec<_> = manifest.split('/').collect();
    parts.pop();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop().ok_or("entrypoint escapes the repository")?;
            }
            part => parts.push(part),
        }
    }
    let path = parts.join("/");
    validate_path(&path)?;
    Ok(path)
}

fn finding(manifest: &str, code: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, message).at(manifest, None)
}

#[cfg(test)]
mod tests;
