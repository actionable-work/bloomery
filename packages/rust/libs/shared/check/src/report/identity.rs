use bloomery_model::Diagnostic;
use bloomery_workspace::source::{MemoryTree, SourceTree, TreeIndex};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub(super) fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// Explicit sorting also supports consumers enabling serde_json's preserve_order.
pub(super) fn identity(value: &impl Serialize) -> String {
    let mut value = serde_json::to_value(value).expect("report values serialize");
    value.sort_all_objects();
    digest(&serde_json::to_vec(&value).expect("JSON values serialize"))
}

#[derive(Serialize)]
pub(super) struct Manifest {
    files: Vec<(String, String)>,
    directories: Vec<String>,
}

impl Manifest {
    pub(super) fn empty() -> Self {
        Self {
            files: Vec::new(),
            directories: Vec::new(),
        }
    }

    pub(super) fn subtree(&self, root: &std::path::Path) -> Self {
        Self {
            files: self
                .files
                .iter()
                .filter(|(path, _)| std::path::Path::new(path).starts_with(root))
                .cloned()
                .collect(),
            directories: self
                .directories
                .iter()
                .filter(|path| std::path::Path::new(path).starts_with(root))
                .cloned()
                .collect(),
        }
    }
}

/// Read each advertised file once; all parsing then uses these exact bytes.
pub(super) fn snapshot(tree: &dyn SourceTree) -> Result<(MemoryTree, Manifest), Diagnostic> {
    let index = TreeIndex::new(tree).map_err(|error| Diagnostic::new("SourceTreeError", error))?;
    let directories: Vec<String> = index.directories().map(String::from).collect();
    let mut files = Vec::new();
    let mut manifest = Manifest {
        files: Vec::new(),
        directories: directories.clone(),
    };
    for path in index.files() {
        let content = tree.read_bytes(path).map_err(|error| {
            Diagnostic::new("SourceTreeError", "Unable to read advertised tree content")
                .at(path, None)
                .note(error)
        })?;
        manifest.files.push((path.to_owned(), digest(&content)));
        files.push((path.to_owned(), content));
    }
    let memory = MemoryTree::from_bytes(files, directories)
        .map_err(|error| Diagnostic::new("SourceTreeError", error))?;
    Ok((memory, manifest))
}
