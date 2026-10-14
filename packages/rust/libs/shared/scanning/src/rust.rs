use crate::ScanOutput;
use crate::analysis::AnalysisSession;
use crate::files::expand_globs;
use bloomery_model::{Diagnostic, Evidence, SourceLocation, TestSite};
use std::path::Path;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};

pub fn scan(
    root: &Path,
    patterns: &[String],
    session: &AnalysisSession,
) -> Result<ScanOutput, Vec<Diagnostic>> {
    let paths = match expand_globs(root, patterns) {
        Ok(paths) => paths,
        Err(error) => return Err(vec![Diagnostic::new("ScannerError", error)]),
    };
    let mut output = ScanOutput::default();
    let mut diagnostics = Vec::new();
    for path in paths {
        let parsed = match session.rust_file(&path) {
            Ok(parsed) => parsed,
            Err(error) => {
                let code = if error.starts_with("Unable to parse")
                    || error.starts_with("Unable to tokenize")
                {
                    "RustParseError"
                } else {
                    "RustScanError"
                };
                diagnostics.push(Diagnostic::new(code, error).at(path, Some(1)));
                continue;
            }
        };
        let mut visitor = Visitor {
            path: &parsed.path,
            output: ScanOutput::default(),
            diagnostics: Vec::new(),
        };
        visitor.visit_file(&parsed.syntax);
        output.absorb(visitor.output);
        diagnostics.extend(visitor.diagnostics);
    }
    if diagnostics.is_empty() {
        Ok(output)
    } else {
        Err(diagnostics)
    }
}

struct Visitor<'a> {
    path: &'a Path,
    output: ScanOutput,
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
            let name = item.sig.ident.to_string();
            let mut references = Vec::new();
            for attribute in &item.attrs {
                if !attribute.path().is_ident("cfg_attr") {
                    continue;
                }
                let nested = match attribute
                    .parse_args_with(Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated)
                {
                    Ok(nested) => nested,
                    Err(_) => continue,
                };
                for meta in nested {
                    let syn::Meta::List(list) = meta else {
                        continue;
                    };
                    if !list.path.is_ident("bloomery") {
                        continue;
                    }
                    match list.parse_args::<syn::LitStr>() {
                        Ok(value) => {
                            let id = value.value();
                            let line = value.span().start().line;
                            self.output.evidence.push(Evidence {
                                id: id.clone(),
                                location: SourceLocation::new(self.path, Some(line)),
                                scanner: "rust",
                            });
                            references.push(id);
                        }
                        Err(error) => self.diagnostics.push(
                            Diagnostic::new(
                                "RustReferenceError",
                                format!(
                                    "Bloomery evidence tag must contain one string ID: {error}"
                                ),
                            )
                            .at(self.path.to_path_buf(), Some(1)),
                        ),
                    }
                }
            }
            let line = item.sig.ident.span().start().line;
            self.output.tests.push(TestSite {
                scanner: "rust",
                name: Some(name),
                location: SourceLocation::new(self.path, Some(line)),
                references,
            });
        }
        visit::visit_item_fn(self, item);
    }
}
