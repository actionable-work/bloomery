//! Authorized source content for static discovery. Paths always use `/`.

use crate::files::{expand_globs, glob_matches, ignored_name};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A stable file index and UTF-8 content for one analysis invocation.
///
/// Implementations must not change while an analysis is running. A listed path
/// must be readable; unavailable content is an error, not an empty file.
pub trait SourceTree {
    fn paths(&self) -> Result<Vec<String>, String>;
    fn read(&self, path: &str) -> Result<String, String>;
}

/// An immutable virtual tree. Construction rejects ambiguous paths.
#[derive(Debug, Clone)]
pub struct MemoryTree {
    files: BTreeMap<String, String>,
}

impl MemoryTree {
    pub fn new<P: Into<String>, S: Into<String>>(
        files: impl IntoIterator<Item = (P, S)>,
    ) -> Result<Self, String> {
        let mut contents = BTreeMap::new();
        for (path, source) in files {
            let path = path.into();
            validate_path(&path)?;
            if contents.insert(path.clone(), source.into()).is_some() {
                return Err(format!("duplicate source path: {path}"));
            }
        }
        validate_index(contents.keys().cloned().collect())?;
        Ok(Self { files: contents })
    }
}

impl SourceTree for MemoryTree {
    fn paths(&self) -> Result<Vec<String>, String> {
        Ok(self.files.keys().cloned().collect())
    }

    fn read(&self, path: &str) -> Result<String, String> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| format!("missing source content: {path}"))
    }
}

/// Local filesystem input. Symbolic links are excluded from discovery.
#[derive(Debug, Clone)]
pub struct FilesystemTree {
    root: PathBuf,
}

impl FilesystemTree {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl SourceTree for FilesystemTree {
    fn paths(&self) -> Result<Vec<String>, String> {
        expand_globs(&self.root, &["**".to_owned()])?
            .iter()
            .map(|path| {
                path.strip_prefix(&self.root)
                    .ok()
                    .and_then(Path::to_str)
                    .map(|path| path.replace(std::path::MAIN_SEPARATOR, "/"))
                    .ok_or_else(|| "source path is not repository-relative UTF-8".to_owned())
            })
            .collect()
    }

    fn read(&self, path: &str) -> Result<String, String> {
        validate_path(path)?;
        fs::read_to_string(self.root.join(path)).map_err(|error| error.to_string())
    }
}

pub(crate) fn validate_path(path: &str) -> Result<(), String> {
    if path.contains(['\\', ':', '\0'])
        || path.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(format!("noncanonical source path: {path}"));
    }
    Ok(())
}

pub(crate) fn validate_index(mut paths: Vec<String>) -> Result<Vec<String>, String> {
    paths.sort();
    let mut seen = std::collections::BTreeSet::new();
    for path in &paths {
        validate_path(path)?;
        if !seen.insert(path.as_str()) {
            return Err(format!("duplicate source path: {path}"));
        }
        for (index, _) in path.match_indices('/') {
            if seen.contains(&path[..index]) {
                return Err(format!("source file/directory collision: {path}"));
            }
        }
    }
    Ok(paths)
}

pub(crate) fn select(paths: &[String], patterns: &[String]) -> Vec<PathBuf> {
    paths
        .iter()
        .filter(|path| {
            !path.split('/').any(ignored_name)
                && patterns
                    .iter()
                    .any(|pattern| glob_matches(pattern, Path::new(path)))
        })
        .map(PathBuf::from)
        .collect()
}
