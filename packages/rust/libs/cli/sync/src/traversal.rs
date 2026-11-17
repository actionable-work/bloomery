use crate::{SyncError, format_diagnostic};
use bloomery_model::config::{LoadedConfig, load_with_raw};
use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

pub(crate) struct Flake {
    pub root: PathBuf,
    pub relative: PathBuf,
    pub has_cargo: bool,
}

impl Flake {
    pub fn lock_path(&self, name: &str) -> String {
        self.relative.join(name).display().to_string()
    }
}

pub(crate) fn discover(root: &Path, config: &LoadedConfig) -> Result<Vec<Flake>, SyncError> {
    let root = root.canonicalize().map_err(|error| {
        SyncError::new(
            "preflight",
            format!(
                "cannot resolve workspace root '{}': {error}",
                root.display()
            ),
        )
    })?;
    let mut flakes = vec![Flake {
        root: root.clone(),
        relative: PathBuf::new(),
        has_cargo: true,
    }];
    visit_children(&root, &root, config, &mut BTreeSet::new(), &mut flakes)?;
    Ok(flakes)
}

fn visit_children(
    root: &Path,
    parent: &Path,
    config: &LoadedConfig,
    seen: &mut BTreeSet<PathBuf>,
    flakes: &mut Vec<Flake>,
) -> Result<(), SyncError> {
    let Some(entries) = config
        .config
        .flakes
        .as_ref()
        .and_then(toml::Value::as_table)
    else {
        return Ok(());
    };
    let mut entries: Vec<_> = entries.iter().collect();
    entries.sort_by_key(|(name, _)| *name);

    for (name, entry) in entries {
        let child = resolve_child(root, parent, name, entry)?;
        if !seen.insert(child.clone()) {
            continue;
        }
        let relative = child.strip_prefix(root).expect("child is inside root");
        flakes.push(Flake {
            root: child.clone(),
            relative: relative.to_path_buf(),
            has_cargo: child.join("Cargo.toml").is_file(),
        });
        if let Some(config) = child_config(&child, relative)? {
            visit_children(root, &child, &config, seen, flakes)?;
        }
    }
    Ok(())
}

fn resolve_child(
    root: &Path,
    parent: &Path,
    name: &str,
    entry: &toml::Value,
) -> Result<PathBuf, SyncError> {
    let relative_parent = parent.strip_prefix(root).expect("parent is inside root");
    let error = |message: String| {
        SyncError::new(
            "sub-flake preflight",
            format!(
                "{}: [flakes.{name}].path {message}",
                Path::new(".").join(relative_parent).display()
            ),
        )
    };
    let path = entry
        .get("path")
        .and_then(toml::Value::as_str)
        .map(Path::new)
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| error("must be a non-empty path".to_owned()))?;
    let child = parent.join(path);
    if path.components().any(|part| {
        matches!(
            part,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(error(format!(
            "'{}' must stay inside its declaring flake",
            path.display()
        )));
    }
    let resolved = child.canonicalize().map_err(|cause| {
        error(format!(
            "'{}' could not be resolved: {cause}",
            path.display()
        ))
    })?;
    if resolved == parent || !resolved.starts_with(parent) {
        return Err(error(format!(
            "'{}' must resolve to a descendant of its declaring flake",
            path.display()
        )));
    }
    if !resolved.join("flake.nix").is_file() {
        return Err(error(format!("'{}' has no flake.nix", path.display())));
    }
    Ok(resolved)
}

fn child_config(child: &Path, relative: &Path) -> Result<Option<LoadedConfig>, SyncError> {
    if fs::symlink_metadata(child.join(".bloomery/config.toml"))
        .is_err_and(|error| error.kind() == ErrorKind::NotFound)
    {
        return Ok(None);
    }

    load_with_raw(child).map(Some).map_err(|diagnostic| {
        SyncError::new(
            "configuration preflight",
            format!(
                "{}: {}",
                relative.display(),
                format_diagnostic(child, &diagnostic)
            ),
        )
    })
}
