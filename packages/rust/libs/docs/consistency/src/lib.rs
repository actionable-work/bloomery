#![allow(clippy::result_large_err)]

mod cli_catalog;
mod coverage;
mod excerpts;
mod glob;
mod links;
mod manifest;

pub use manifest::{Manifest, Surface, load_manifest};

use bloomery_model::Diagnostic;
use std::path::Path;

/// Validate the repository documentation against the coverage manifest.
///
/// The manifest and the specification tree are read from `root`. A missing or
/// unreadable manifest is a setup error; every specification violation is a
/// diagnostic.
pub fn check(root: &Path) -> Result<Vec<Diagnostic>, String> {
    let manifest = load_manifest(root)?;
    let features = discover_features(root)?;
    Ok(validate(root, &manifest, &features))
}

/// Validate a loaded manifest against an explicit feature set.
///
/// This is the pure entry point used by tests; it still reads documentation
/// files below `root`.
pub fn validate(root: &Path, manifest: &Manifest, features: &[String]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(coverage::check(root, manifest, features));
    diagnostics.extend(links::check(root, manifest));
    diagnostics.extend(excerpts::check(root, manifest));
    bloomery_model::sort_diagnostics(&mut diagnostics);
    diagnostics
}

fn discover_features(root: &Path) -> Result<Vec<String>, String> {
    let config = bloomery_model::config::load(root).map_err(|error| error.to_string())?;
    let specs_root = config.specs_root(root).map_err(|error| error.to_string())?;
    if !specs_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut features = Vec::new();
    for area in child_directories(&specs_root)? {
        let Some(area_id) = directory_name(&area) else {
            continue;
        };
        for feature in child_directories(&area)? {
            let Some(feature_id) = directory_name(&feature) else {
                continue;
            };
            features.push(format!("{area_id}/{feature_id}"));
        }
    }
    features.sort();
    Ok(features)
}

fn child_directories(path: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut directories = Vec::new();
    let entries = std::fs::read_dir(path)
        .map_err(|error| format!("unable to read {}: {error}", path.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("unable to read {}: {error}", path.display()))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("unable to inspect {}: {error}", entry.path().display()))?;
        if file_type.is_dir() {
            directories.push(entry.path());
        }
    }
    directories.sort();
    Ok(directories)
}

fn directory_name(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    pub(crate) struct TempRoot {
        pub(crate) path: PathBuf,
    }

    impl TempRoot {
        pub(crate) fn new(label: &str) -> Self {
            let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "bloomery-docs-consistency-{}-{label}-{suffix}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("temp root");
            Self { path }
        }

        pub(crate) fn write(&self, relative: &str, contents: &str) -> PathBuf {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("parent directory");
            }
            std::fs::write(&path, contents).expect("write fixture");
            path
        }

        pub(crate) fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempRoot;

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-CONSISTENCY-CHECK-002"))]
    fn check_reports_a_repository_violation() {
        let fixture = TempRoot::new("check-violation");
        fixture.write(".bloomery/config.toml", "[specs]\ndir = \"specs\"\n");
        fixture.write(
            ".bloomery/specs/EXAMPLE/FEATURE/README.md",
            "---\nid: FEATURE\nname: Feature\ntagline: t\ndescription: d\n---\n",
        );
        fixture.write(
            ".bloomery/specs/EXAMPLE/FEATURE/design/overview.md",
            "# Overview\n",
        );
        fixture.write(
            ".bloomery/specs/EXAMPLE/FEATURE/requirements/GROUP.toml",
            "group = \"GROUP\"\n",
        );
        fixture.write("README.md", "# Docs\n");
        fixture.write(
            ".bloomery/docs.manifest.toml",
            "roots = [\"README.md\"]\n\n[[surfaces]]\npath = \"README.md\"\nfeatures = []\n",
        );

        let diagnostics = check(fixture.path()).expect("check should run");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "UndocumentedFeature"),
            "{diagnostics:?}"
        );
    }
}
