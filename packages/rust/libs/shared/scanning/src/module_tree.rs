//! Rust source files reachable from a target root through `mod` declarations.

use crate::AnalysisSession;
use crate::analysis::ParsedRustFile;
use crate::modules::{module_child_dir, module_paths};
use proc_macro2::TokenStream;
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
    module_tree_scopes(entry, session, visited, &|_| false)
        .into_iter()
        .map(|file| file.map(|(parsed, _)| parsed))
        .collect()
}

/// [`module_tree_files`] paired with whether each file is confined: its inner
/// attributes, a declaring `mod` item, an enclosing inline module, or an
/// ancestor file carry a `cfg(predicate)` for which `confines` holds.
pub fn module_tree_scopes(
    entry: &Path,
    session: &AnalysisSession,
    visited: &mut BTreeSet<PathBuf>,
    confines: &dyn Fn(&TokenStream) -> bool,
) -> Vec<Result<(Rc<ParsedRustFile>, bool), ModuleSourceError>> {
    let mut results = Vec::new();
    let entry_dir = entry.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut pending = vec![(entry.to_path_buf(), entry_dir, false)];
    while let Some((path, module_dir, inherited)) = pending.pop() {
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
        let confined = inherited || is_confined(&parsed.syntax.attrs, confines);
        let mut collector = ModuleCollector {
            module_dir,
            confined,
            confines,
            children: Vec::new(),
        };
        collector.visit_file(&parsed.syntax);
        pending.extend(collector.children.into_iter().rev());
        results.push(Ok((parsed, confined)));
    }
    results
}

fn is_confined(attrs: &[syn::Attribute], confines: &dyn Fn(&TokenStream) -> bool) -> bool {
    attrs.iter().any(|attr| {
        matches!(&attr.meta, syn::Meta::List(list) if list.path.is_ident("cfg") && confines(&list.tokens))
    })
}

struct ModuleCollector<'a> {
    module_dir: PathBuf,
    confined: bool,
    confines: &'a dyn Fn(&TokenStream) -> bool,
    children: Vec<(PathBuf, PathBuf, bool)>,
}

impl<'ast> Visit<'ast> for ModuleCollector<'_> {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        let confined = self.confined || is_confined(&item.attrs, self.confines);
        if item.content.is_some() {
            let previous = (self.module_dir.clone(), self.confined);
            self.module_dir = previous.0.join(item.ident.to_string());
            self.confined = confined;
            visit::visit_item_mod(self, item);
            (self.module_dir, self.confined) = previous;
            return;
        }
        for path in module_paths(item, &self.module_dir, Path::is_file) {
            if path.is_file() {
                let child_dir = module_child_dir(&path);
                self.children.push((path, child_dir, confined));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{module_tree_files, module_tree_scopes};
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

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-013"))]
    fn confining_cfg_modules_and_files_are_scoped() {
        let confines = |predicate: &proc_macro2::TokenStream| {
            let predicate = predicate.to_string();
            predicate == "test" || predicate.starts_with("all (test")
        };
        let root = std::env::temp_dir().join("bloomery-module-tree-scopes");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src/tests")).expect("directories");
        fs::write(
            root.join("src/lib.rs"),
            "mod plain;\n#[cfg(test)] mod tests;\nmod inner_only;\n#[cfg(all(test, unix))] mod wrapped { mod deep; }\n",
        )
        .expect("lib");
        fs::write(root.join("src/plain.rs"), "").expect("plain");
        fs::write(root.join("src/tests.rs"), "mod child;\n").expect("tests");
        fs::write(root.join("src/tests/child.rs"), "").expect("child");
        fs::write(root.join("src/inner_only.rs"), "#![cfg(test)]\n").expect("inner");
        fs::create_dir_all(root.join("src/wrapped")).expect("wrapped");
        fs::write(root.join("src/wrapped/deep.rs"), "").expect("deep");
        let canonical = fs::canonicalize(&root).expect("root");
        let scopes = module_tree_scopes(
            &root.join("src/lib.rs"),
            &AnalysisSession::new(),
            &mut BTreeSet::new(),
            &confines,
        )
        .into_iter()
        .map(|file| {
            let (parsed, confined) = file.expect("parsed");
            let path = parsed
                .path
                .strip_prefix(&canonical)
                .expect("inside root")
                .to_string_lossy()
                .into_owned();
            (path, confined)
        })
        .collect::<BTreeSet<_>>();
        assert_eq!(
            scopes,
            BTreeSet::from([
                ("src/inner_only.rs".to_owned(), true),
                ("src/lib.rs".to_owned(), false),
                ("src/plain.rs".to_owned(), false),
                ("src/tests.rs".to_owned(), true),
                ("src/tests/child.rs".to_owned(), true),
                ("src/wrapped/deep.rs".to_owned(), true),
            ])
        );
        let _ = fs::remove_dir_all(root);
    }
}
