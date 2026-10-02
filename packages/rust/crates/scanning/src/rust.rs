use crate::files::expand_globs;
use bloomery_model::{Diagnostic, Evidence, SourceLocation};
use std::fs;
use std::path::Path;
use syn::visit::{self, Visit};

pub fn scan(root: &Path, patterns: &[String]) -> Result<Vec<Evidence>, Vec<Diagnostic>> {
    let paths = match expand_globs(root, patterns) {
        Ok(paths) => paths,
        Err(error) => return Err(vec![Diagnostic::new("ScannerError", error)]),
    };
    let mut evidence = Vec::new();
    let mut diagnostics = Vec::new();
    for path in paths {
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) => {
                diagnostics.push(
                    Diagnostic::new(
                        "RustScanError",
                        format!("Unable to read Rust source: {error}"),
                    )
                    .at(path, Some(1)),
                );
                continue;
            }
        };
        let syntax = match syn::parse_file(&contents) {
            Ok(syntax) => syntax,
            Err(error) => {
                diagnostics.push(
                    Diagnostic::new(
                        "RustParseError",
                        format!("Unable to parse Rust source: {error}"),
                    )
                    .at(path, Some(1)),
                );
                continue;
            }
        };
        let mut visitor = Visitor {
            path: &path,
            contents: &contents,
            evidence: Vec::new(),
            diagnostics: Vec::new(),
        };
        visitor.visit_file(&syntax);
        evidence.extend(visitor.evidence);
        diagnostics.extend(visitor.diagnostics);
    }
    if diagnostics.is_empty() {
        Ok(evidence)
    } else {
        Err(diagnostics)
    }
}

struct Visitor<'a> {
    path: &'a Path,
    contents: &'a str,
    evidence: Vec<Evidence>,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for Visitor<'_> {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let is_test = item.attrs.iter().any(|attribute| {
            attribute.path().is_ident("test")
                || attribute
                    .path()
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "test")
        });
        if is_test {
            for attribute in &item.attrs {
                if !attribute.path().is_ident("bloomery") {
                    continue;
                }
                match attribute.parse_args::<syn::LitStr>() {
                    Ok(value) => {
                        let needle = format!("bloomery(\"{}\"", value.value());
                        let line = self
                            .contents
                            .find(&needle)
                            .map(|offset| self.contents[..offset].lines().count() + 1)
                            .unwrap_or(1);
                        self.evidence.push(Evidence {
                            id: value.value(),
                            location: SourceLocation::new(self.path, Some(line)),
                            scanner: "rust",
                        });
                    }
                    Err(error) => self.diagnostics.push(
                        Diagnostic::new(
                            "RustReferenceError",
                            format!("Bloomery attribute must contain one string ID: {error}"),
                        )
                        .at(self.path.to_path_buf(), Some(1)),
                    ),
                }
            }
        }
        visit::visit_item_fn(self, item);
    }
}
