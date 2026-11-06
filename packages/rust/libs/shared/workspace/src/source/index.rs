use super::{
    SourceTree, files::glob_matches, ignored_name, path_key, validate_index, validate_path,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Validated repository paths with explicit and inferred directories.
pub struct TreeIndex {
    files: BTreeSet<String>,
    directories: BTreeSet<String>,
}

impl TreeIndex {
    /// Sorted canonical paths after applying the shared discovery exclusions.
    pub fn files(&self) -> impl Iterator<Item = &str> {
        self.files.iter().map(String::as_str)
    }

    /// Sorted explicit and inferred directory paths.
    pub fn directories(&self) -> impl Iterator<Item = &str> {
        self.directories.iter().map(String::as_str)
    }

    pub fn new(tree: &dyn SourceTree) -> Result<Self, String> {
        let files: BTreeSet<_> = validate_index(tree.paths()?)?.into_iter().collect();
        let mut directories = BTreeSet::new();
        for path in tree.directories()? {
            validate_path(&path)?;
            if !directories.insert(path.clone()) {
                return Err(format!("duplicate directory path: {path}"));
            }
        }
        let explicit = directories.clone();
        for path in files.iter().chain(&explicit) {
            for (index, _) in path.match_indices('/') {
                directories.insert(path[..index].to_owned());
            }
        }
        if let Some(path) = directories.iter().find(|path| files.contains(*path)) {
            return Err(format!("source file/directory collision: {path}"));
        }
        Ok(Self {
            files: files.into_iter().filter(|path| !ignored(path)).collect(),
            directories: directories
                .into_iter()
                .filter(|path| !ignored(path))
                .collect(),
        })
    }

    pub fn is_file(&self, path: &Path) -> bool {
        self.files.contains(&path_key(path))
    }

    pub fn is_dir(&self, path: &Path) -> bool {
        path.as_os_str().is_empty() || self.directories.contains(&path_key(path))
    }

    pub fn child_directories(&self, path: &Path) -> Vec<PathBuf> {
        self.directories
            .iter()
            .map(PathBuf::from)
            .filter(|child| child.parent() == Some(path))
            .collect()
    }

    pub fn child_files(&self, path: &Path) -> Vec<PathBuf> {
        self.files
            .iter()
            .map(PathBuf::from)
            .filter(|child| child.parent() == Some(path))
            .collect()
    }

    pub fn matching_paths(&self, patterns: &[String]) -> Vec<PathBuf> {
        self.files
            .iter()
            .filter(|path| {
                patterns
                    .iter()
                    .any(|pattern| glob_matches(pattern, Path::new(path)))
            })
            .map(PathBuf::from)
            .collect()
    }
}

fn ignored(path: &str) -> bool {
    path.split('/').any(ignored_name)
}
