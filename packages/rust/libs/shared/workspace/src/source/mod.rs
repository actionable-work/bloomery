//! Authorized source content for static discovery. Paths always use `/`.

mod files;
mod index;
use files::{filesystem_entries, ignored_name};
pub use index::TreeIndex;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A stable file index and content for one analysis invocation.
///
/// Implementations must not change while an analysis is running. A listed path
/// must be readable; unavailable content is an error, not an empty file.
pub trait SourceTree {
    fn paths(&self) -> Result<Vec<String>, String>;
    fn read(&self, path: &str) -> Result<String, String>;
    /// Raw content for snapshot identities. Text-only adapters need no override.
    fn read_bytes(&self, path: &str) -> Result<Vec<u8>, String> {
        self.read(path).map(String::into_bytes)
    }
    /// Explicit directories, including empty ones. File ancestors are inferred.
    fn directories(&self) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
}

/// An immutable virtual tree. Construction rejects ambiguous paths.
#[derive(Debug, Clone)]
pub struct MemoryTree {
    files: BTreeMap<String, Vec<u8>>,
    directories: Vec<String>,
}

impl MemoryTree {
    pub fn new<P: Into<String>, S: Into<String>>(
        files: impl IntoIterator<Item = (P, S)>,
    ) -> Result<Self, String> {
        Self::with_directories(files, std::iter::empty::<String>())
    }

    pub fn with_directories<P: Into<String>, S: Into<String>, D: Into<String>>(
        files: impl IntoIterator<Item = (P, S)>,
        directories: impl IntoIterator<Item = D>,
    ) -> Result<Self, String> {
        Self::from_bytes(
            files
                .into_iter()
                .map(|(path, source)| (path, source.into().into_bytes())),
            directories,
        )
    }

    /// Construct a byte-preserving snapshot, including explicit empty directories.
    pub fn from_bytes<P: Into<String>, D: Into<String>>(
        files: impl IntoIterator<Item = (P, Vec<u8>)>,
        directories: impl IntoIterator<Item = D>,
    ) -> Result<Self, String> {
        let mut contents = BTreeMap::new();
        for (path, source) in files {
            let path = path.into();
            validate_path(&path)?;
            if contents.insert(path.clone(), source).is_some() {
                return Err(format!("duplicate source path: {path}"));
            }
        }
        let tree = Self {
            files: contents,
            directories: directories.into_iter().map(Into::into).collect(),
        };
        TreeIndex::new(&tree)?;
        Ok(tree)
    }
}

impl SourceTree for MemoryTree {
    fn directories(&self) -> Result<Vec<String>, String> {
        Ok(self.directories.clone())
    }

    fn paths(&self) -> Result<Vec<String>, String> {
        Ok(self.files.keys().cloned().collect())
    }

    fn read(&self, path: &str) -> Result<String, String> {
        String::from_utf8(self.read_bytes(path)?)
            .map_err(|_| format!("source content is not UTF-8: {path}"))
    }

    fn read_bytes(&self, path: &str) -> Result<Vec<u8>, String> {
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
        Ok(filesystem_entries(&self.root)?.0)
    }

    fn directories(&self) -> Result<Vec<String>, String> {
        Ok(filesystem_entries(&self.root)?.1)
    }

    fn read(&self, path: &str) -> Result<String, String> {
        validate_path(path)?;
        fs::read_to_string(self.root.join(path)).map_err(|error| error.to_string())
    }

    fn read_bytes(&self, path: &str) -> Result<Vec<u8>, String> {
        validate_path(path)?;
        fs::read(self.root.join(path)).map_err(|error| error.to_string())
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

/// Canonical key for already validated repository-relative paths.
pub(crate) fn path_key(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Resolve a portable relative path inside an authorized repository directory.
pub fn resolve_relative_path(directory: &str, path: &str) -> Result<String, String> {
    if path.is_empty() || path.starts_with('/') || path.contains(['\\', ':', '\0']) {
        return Err("entrypoint must be a relative portable path".into());
    }
    if !directory.is_empty() {
        validate_path(directory)?;
    }
    let mut parts: Vec<_> = if directory.is_empty() {
        Vec::new()
    } else {
        directory.split('/').collect()
    };
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
