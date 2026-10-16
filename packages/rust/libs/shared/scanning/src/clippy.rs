use crate::AnalysisSession;
use bloomery_model::{Diagnostic, sort_diagnostics};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Meta, punctuated::Punctuated};

/// Find inline attributes that disable Clippy lints outside the configured
/// exact-name exception set.
pub fn find_clippy_suppressions(
    root: &Path,
    source_paths: &[PathBuf],
    allowed: &[String],
    session: &AnalysisSession,
) -> Result<(), Vec<Diagnostic>> {
    let root = std::fs::canonicalize(root).map_err(|error| {
        vec![Diagnostic::new(
            "ClippySourceError",
            format!("Unable to resolve repository root: {error}"),
        )]
    })?;
    let allowed = allowed.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let mut diagnostics = Vec::new();
    let mut paths = source_paths.to_vec();
    paths.sort();
    paths.dedup();
    let mut visited = BTreeSet::new();
    for path in paths {
        let module_dir = path.parent().unwrap_or(&root);
        scan_source_file(
            &root,
            &path,
            module_dir,
            &allowed,
            session,
            &mut visited,
            &mut diagnostics,
        );
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        sort_diagnostics(&mut diagnostics);
        Err(diagnostics)
    }
}

fn scan_source_file(
    root: &Path,
    path: &Path,
    module_dir: &Path,
    allowed: &BTreeSet<&str>,
    session: &AnalysisSession,
    visited: &mut BTreeSet<PathBuf>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let parsed = match session.rust_file(path) {
        Ok(parsed) => parsed,
        Err(error) => {
            diagnostics.push(Diagnostic::new("ClippySourceError", error).at(path, None));
            return;
        }
    };
    if !visited.insert(parsed.path.clone()) {
        return;
    }
    let relative = match parsed.path.strip_prefix(root) {
        Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
        Err(_) => {
            diagnostics.push(Diagnostic::new(
                "ClippySourceError",
                "Rust target source resolves outside the repository",
            ));
            return;
        }
    };
    let modules = {
        let mut visitor = SuppressionVisitor {
            path: relative,
            module_dir: module_dir.to_path_buf(),
            allowed,
            diagnostics,
            modules: Vec::new(),
        };
        visitor.visit_file(&parsed.syntax);
        visitor.modules
    };
    for (module_path, child_dir) in modules {
        scan_source_file(
            root,
            &module_path,
            &child_dir,
            allowed,
            session,
            visited,
            diagnostics,
        );
    }
}

struct SuppressionVisitor<'a> {
    path: String,
    module_dir: PathBuf,
    allowed: &'a BTreeSet<&'a str>,
    diagnostics: &'a mut Vec<Diagnostic>,
    modules: Vec<(PathBuf, PathBuf)>,
}

impl<'ast> Visit<'ast> for SuppressionVisitor<'_> {
    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        let line = attribute.span().start().line;
        self.inspect_meta(&attribute.meta, line);
        visit::visit_attribute(self, attribute);
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        for attribute in &item.attrs {
            self.visit_attribute(attribute);
        }
        if let Some((_, items)) = &item.content {
            let previous = self.module_dir.clone();
            self.module_dir = previous.join(item.ident.to_string());
            for child in items {
                self.visit_item(child);
            }
            self.module_dir = previous;
            return;
        }
        let name = item.ident.to_string();
        let custom = item
            .attrs
            .iter()
            .find_map(|attribute| match &attribute.meta {
                Meta::NameValue(value) if value.path.is_ident("path") => {
                    if let syn::Expr::Lit(expression) = &value.value
                        && let syn::Lit::Str(path) = &expression.lit
                    {
                        Some(self.module_dir.join(path.value()))
                    } else {
                        None
                    }
                }
                _ => None,
            });
        let candidates = if let Some(path) = custom {
            vec![path]
        } else {
            let first = self.module_dir.join(format!("{name}.rs"));
            let second = self.module_dir.join(&name).join("mod.rs");
            let existing = [first.clone(), second]
                .into_iter()
                .filter(|path| path.is_file())
                .collect::<Vec<_>>();
            if existing.is_empty() {
                vec![first]
            } else {
                existing
            }
        };
        for module_path in candidates {
            if module_path.is_file() {
                let child_dir = if module_path.file_name().is_some_and(|name| name == "mod.rs") {
                    module_path
                        .parent()
                        .unwrap_or(&self.module_dir)
                        .to_path_buf()
                } else {
                    module_path
                        .parent()
                        .unwrap_or(&self.module_dir)
                        .join(module_path.file_stem().unwrap_or_default())
                };
                self.modules.push((module_path, child_dir));
            }
        }
    }
}

impl SuppressionVisitor<'_> {
    fn inspect_meta(&mut self, meta: &Meta, line: usize) {
        match meta {
            Meta::List(list) if list.path.is_ident("allow") => {
                if let Ok(lints) =
                    list.parse_args_with(Punctuated::<Meta, syn::Token![,]>::parse_terminated)
                {
                    for lint in &lints {
                        self.report_lint(lint, line);
                    }
                }
            }
            Meta::List(list) if list.path.is_ident("cfg_attr") => {
                if let Ok(arguments) =
                    list.parse_args_with(Punctuated::<Meta, syn::Token![,]>::parse_terminated)
                {
                    for nested in arguments.iter().skip(1) {
                        self.inspect_meta(nested, line);
                    }
                }
            }
            _ => {}
        }
    }

    fn report_lint(&mut self, meta: &Meta, line: usize) {
        let path = meta.path();
        let segments = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
        if segments.len() != 2 || segments[0] != "clippy" {
            return;
        }
        let lint = format!("{}::{}", segments[0], segments[1]);
        if !self.allowed.contains(lint.as_str()) {
            self.diagnostics.push(
                Diagnostic::new(
                    "ClippySuppression",
                    format!("inline attribute disables Clippy lint '{lint}'"),
                )
                .at(Path::new(&self.path).to_path_buf(), Some(line)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::find_clippy_suppressions;
    use crate::AnalysisSession;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static FIXTURE_ID: AtomicUsize = AtomicUsize::new(0);

    fn fixture(source: &str) -> (PathBuf, PathBuf) {
        let id = FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("bloomery-clippy-suppressions-{id}"));
        fs::create_dir_all(root.join("src")).expect("source directory");
        let path = root.join("src/lib.rs");
        fs::write(&path, source).expect("source");
        (root, path)
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-SUPPRESSIONS-003"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-SUPPRESSIONS-005"))]
    fn reports_disallowed_lints_with_path_and_attribute_line() {
        let (root, path) =
            fixture("pub fn f() {}\n#[allow(clippy::unwrap_used, clippy::all)]\npub fn g() {}\n");
        let diagnostics = find_clippy_suppressions(&root, &[path], &[], &AnalysisSession::new())
            .expect_err("both lints should be rejected");
        assert_eq!(diagnostics.len(), 2);
        assert!(
            diagnostics
                .iter()
                .all(|item| item.location.as_ref().is_some_and(|location| {
                    location.path.ends_with("src/lib.rs") && location.line == Some(2)
                }))
        );
        assert!(
            diagnostics
                .iter()
                .any(|item| item.message.contains("clippy::unwrap_used"))
        );
        assert!(
            diagnostics
                .iter()
                .any(|item| item.message.contains("clippy::all"))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-SUPPRESSIONS-004"))]
    fn configured_names_are_exact_exceptions_and_cfg_attr_is_inspected() {
        let (root, path) = fixture(
            "#![allow(clippy::all)]\n#[cfg_attr(test, allow(clippy::unwrap_used))]\npub fn f() {}\n",
        );
        let diagnostics = find_clippy_suppressions(
            &root,
            &[path],
            &["clippy::unwrap_used".to_owned()],
            &AnalysisSession::new(),
        )
        .expect_err("group name remains a separate lint");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("clippy::all"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-SUPPRESSIONS-004"))]
    fn allows_exact_lint_name_wherever_it_is_suppressed() {
        let (root, path) = fixture(
            "#![allow(clippy::unwrap_used)]\n#[cfg_attr(test, allow(clippy::unwrap_used))]\npub fn f() {}\n",
        );
        find_clippy_suppressions(
            &root,
            &[path],
            &["clippy::unwrap_used".to_owned()],
            &AnalysisSession::new(),
        )
        .expect("configured lint is allowed");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-SUPPRESSIONS-002"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-SUPPRESSIONS-005"))]
    fn scans_out_of_line_modules_recursively() {
        let (root, path) = fixture("mod nested;\n");
        fs::write(
            root.join("src/nested.rs"),
            "#[allow(clippy::all)]\npub fn nested() {}\n",
        )
        .expect("nested source");
        let diagnostics = find_clippy_suppressions(&root, &[path], &[], &AnalysisSession::new())
            .expect_err("nested module suppression should be found");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].location.as_ref().is_some_and(|location| {
            location.path.ends_with("src/nested.rs") && location.line == Some(1)
        }));
        let _ = fs::remove_dir_all(root);
    }
}
