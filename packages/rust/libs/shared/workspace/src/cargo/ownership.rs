//! Manifest-only workspace selection for specification ownership.

use crate::source::{SourceTree, TreeIndex, resolve_relative_path, validate_path};
use bloomery_model::{Config, Diagnostic};
use std::collections::BTreeSet;
use std::path::Path;

pub struct WorkspaceCrate {
    pub manifest: String,
    pub name: String,
}

pub fn selected_crates(
    tree: &dyn SourceTree,
    config: &Config,
) -> Result<Vec<WorkspaceCrate>, Diagnostic> {
    let manifest = config
        .build
        .as_ref()
        .and_then(|build| build.get("cargoToml"))
        .and_then(|value| value.as_str())
        .unwrap_or("Cargo.toml");
    validate_path(manifest).map_err(|error| failure(manifest, error))?;
    let index = TreeIndex::new(tree).map_err(|error| failure(manifest, error))?;
    if !index.is_file(Path::new(manifest)) {
        return Err(failure(
            manifest,
            "workspace manifest is not an indexed file",
        ));
    }
    let root = read_manifest(tree, manifest)?;
    if root.get("package").is_none() && root.get("workspace").is_none() {
        return Err(failure(manifest, "expected a package or workspace table"));
    }
    if root
        .get("workspace")
        .is_some_and(|workspace| !workspace.is_table())
    {
        return Err(failure(manifest, "workspace must be a table"));
    }
    let directory = manifest
        .rsplit_once('/')
        .map(|(directory, _)| directory)
        .unwrap_or("");
    let mut paths = BTreeSet::new();
    if root.get("package").is_some() {
        paths.insert(manifest.to_owned());
    }
    let patterns = strings(
        root.get("workspace")
            .and_then(|workspace| workspace.get("members")),
        manifest,
    )?;
    let excludes = strings(
        root.get("workspace")
            .and_then(|workspace| workspace.get("exclude")),
        manifest,
    )?;
    let excludes: Vec<_> = excludes
        .iter()
        .map(|exclude| resolve_relative_path(directory, exclude))
        .collect::<Result<_, _>>()
        .map_err(|error| failure(manifest, error))?;
    for pattern in patterns {
        let member =
            resolve_relative_path(directory, &pattern).map_err(|error| failure(manifest, error))?;
        let glob = if member.is_empty() {
            "Cargo.toml".into()
        } else {
            format!("{member}/Cargo.toml")
        };
        let matched = index.matching_paths(&[glob]);
        if matched.is_empty() {
            return Err(failure(
                manifest,
                format!("workspace member has no manifest: {pattern}"),
            ));
        }
        for path in matched {
            let directory = path.parent().unwrap_or(Path::new(""));
            if !excludes
                .iter()
                .any(|exclude| bloomery_glob::path_matches(exclude, directory))
            {
                paths.insert(path.to_string_lossy().into_owned());
            }
        }
    }
    let selection = config.build.as_ref().and_then(|build| build.get("members"));
    let names = strings(selection, manifest)?;
    let mut crates = Vec::new();
    for path in paths {
        let document = if path == manifest {
            root.clone()
        } else {
            read_manifest(tree, &path)?
        };
        let name = document
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(toml::Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| failure(&path, "workspace member needs a package.name"))?;
        if selection.is_none() || names.iter().any(|selected| selected == name) {
            crates.push(WorkspaceCrate {
                manifest: path,
                name: name.into(),
            });
        }
    }
    for name in names {
        if !crates.iter().any(|package| package.name == name) {
            return Err(failure(
                manifest,
                format!("build.members names an unknown workspace crate: {name}"),
            ));
        }
    }
    Ok(crates)
}

fn strings(value: Option<&toml::Value>, manifest: &str) -> Result<Vec<String>, Diagnostic> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    value
        .as_array()
        .ok_or_else(|| failure(manifest, "member selection must be a string list"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| failure(manifest, "member selection must be a string list"))
        })
        .collect()
}

fn read_manifest(tree: &dyn SourceTree, manifest: &str) -> Result<toml::Value, Diagnostic> {
    let source = tree
        .read(manifest)
        .map_err(|error| failure(manifest, error))?;
    toml::from_str(&source)
        .map_err(|error| failure(manifest, format!("invalid Cargo manifest: {error}")))
}

fn failure(manifest: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("SpecificationPolicyError", message).at(manifest, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::MemoryTree;

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-WORKSPACE-QUALITY-002"))]
    fn custom_workspace_roots_and_crate_name_selection_ignore_unrelated_workspaces() {
        let tree = MemoryTree::new([
            ("project/Cargo.toml", "[package]\nname = 'root'\n[workspace]\nmembers = ['crates/*']\nexclude = ['crates/fixture']"),
            ("project/crates/app/Cargo.toml", "[package]\nname = 'app'"),
            ("project/crates/fixture/Cargo.toml", "invalid TOML ["),
            ("other/Cargo.toml", "invalid TOML ["),
        ]).unwrap();
        let mut config = Config {
            build: Some(
                toml::from_str("cargoToml = 'project/Cargo.toml'\nmembers = ['app']").unwrap(),
            ),
            ..Config::default()
        };
        let selected = selected_crates(&tree, &config).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].manifest, "project/crates/app/Cargo.toml");
        config.build = Some(toml::from_str("cargoToml = 'project/Cargo.toml'").unwrap());
        assert_eq!(selected_crates(&tree, &config).unwrap().len(), 2);
        config.build = Some(
            toml::from_str("cargoToml = 'project/Cargo.toml'\nmembers = ['missing']").unwrap(),
        );
        assert_eq!(
            selected_crates(&tree, &config).err().unwrap().code,
            "SpecificationPolicyError"
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-QUALITY-009"))]
    fn unavailable_or_malformed_workspace_manifests_fail_explicitly() {
        for source in [
            "unrelated = true",
            "workspace = 'invalid'",
            "[workspace]\nmembers = ['missing']",
            "invalid TOML [",
            "[workspace]\nmembers = [1]",
        ] {
            let tree = MemoryTree::new([("Cargo.toml", source)]).unwrap();
            assert_eq!(
                selected_crates(&tree, &Config::default())
                    .err()
                    .unwrap()
                    .code,
                "SpecificationPolicyError"
            );
        }
        let tree = MemoryTree::new(std::iter::empty::<(&str, &str)>()).unwrap();
        assert!(selected_crates(&tree, &Config::default()).is_err());
    }
}
