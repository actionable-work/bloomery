//! Rust source files reachable from a target root through `mod` declarations.

use crate::AnalysisSession;
use crate::analysis::ParsedRustFile;
use crate::modules::{module_child_dir, module_paths};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use syn::visit::{self, Visit};

/// A source file in a module tree that could not be read or parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleSourceError {
    pub path: PathBuf,
    pub message: String,
}

/// Parse `entry` and every out-of-line module file it declares, recursively.
///
/// Files already in `visited` (canonical paths) are skipped, so callers can
/// share one set across targets to scan each file once. Declared module files
/// that do not exist are ignored; the compiler reports those.
pub fn module_tree_files(
    entry: &Path,
    session: &AnalysisSession,
    visited: &mut BTreeSet<PathBuf>,
) -> Vec<Result<Rc<ParsedRustFile>, ModuleSourceError>> {
    let mut results = Vec::new();
    let entry_dir = entry.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut pending = vec![(entry.to_path_buf(), entry_dir)];
    while let Some((path, module_dir)) = pending.pop() {
        let parsed = match session.rust_file(&path) {
            Ok(parsed) => parsed,
            Err(message) => {
                results.push(Err(ModuleSourceError { path, message }));
                continue;
            }
        };
        if !visited.insert(parsed.path.clone()) {
            continue;
        }
        let mut collector = ModuleCollector {
            module_dir,
            children: Vec::new(),
        };
        collector.visit_file(&parsed.syntax);
        pending.extend(collector.children.into_iter().rev());
        results.push(Ok(parsed));
    }
    results
}

struct ModuleCollector {
    module_dir: PathBuf,
    children: Vec<(PathBuf, PathBuf)>,
}

impl<'ast> Visit<'ast> for ModuleCollector {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if item.content.is_some() {
            let previous = self.module_dir.clone();
            self.module_dir = previous.join(item.ident.to_string());
            visit::visit_item_mod(self, item);
            self.module_dir = previous;
            return;
        }
        for path in module_paths(item, &self.module_dir, Path::is_file) {
            if path.is_file() {
                let child_dir = module_child_dir(&path);
                self.children.push((path, child_dir));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::module_tree_files;
    use crate::AnalysisSession;
    use std::collections::BTreeSet;
    use std::fs;

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-SUPPRESSIONS-002"))]
    fn walks_out_of_line_inline_and_path_modules_once() {
        let root = std::env::temp_dir().join("bloomery-module-tree-walk");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src/outer/inner")).expect("directories");
        fs::write(
            root.join("src/lib.rs"),
            "mod a;\nmod outer { mod inner; }\n#[path = \"custom.rs\"] mod renamed;\nmod missing;\n",
        )
        .expect("lib");
        fs::write(root.join("src/a.rs"), "mod b;\n").expect("a");
        fs::create_dir_all(root.join("src/a")).expect("a directory");
        fs::write(root.join("src/a/b.rs"), "").expect("b");
        fs::write(root.join("src/outer/inner.rs"), "").expect("inner");
        fs::write(root.join("src/custom.rs"), "").expect("custom");
        let session = AnalysisSession::new();
        let mut visited = BTreeSet::new();
        let files = module_tree_files(&root.join("src/lib.rs"), &session, &mut visited)
            .into_iter()
            .map(|file| {
                let path = file.expect("parsed").path.clone();
                let relative = path
                    .strip_prefix(fs::canonicalize(&root).expect("root"))
                    .expect("inside root")
                    .to_string_lossy()
                    .into_owned();
                relative
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            files,
            BTreeSet::from(
                [
                    "src/lib.rs",
                    "src/a.rs",
                    "src/a/b.rs",
                    "src/outer/inner.rs",
                    "src/custom.rs"
                ]
                .map(str::to_owned)
            )
        );
        assert!(module_tree_files(&root.join("src/a.rs"), &session, &mut visited).is_empty());
        let _ = fs::remove_dir_all(root);
    }
}
