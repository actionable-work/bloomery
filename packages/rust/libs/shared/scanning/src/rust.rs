use crate::ScanOutput;
use crate::SourceTree;
use crate::analysis::AnalysisSession;
use crate::positions::SourceMap;
use bloomery_model::{Diagnostic, Evidence, SourceLocation, TestSite};
use std::path::Path;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};

pub fn scan(
    tree: &dyn SourceTree,
    paths: &[std::path::PathBuf],
    session: &AnalysisSession,
    filesystem_root: Option<&Path>,
) -> Result<ScanOutput, Vec<Diagnostic>> {
    let mut output = ScanOutput::default();
    let mut diagnostics = Vec::new();
    for path in paths {
        let parsed = match filesystem_root {
            Some(root) => session.rust_file(&root.join(path)),
            None => session.rust_source_file(path, || {
                tree.read(path.to_str().expect("validated path"))
                    .map_err(|error| format!("Unable to read Rust source: {error}"))
            }),
        };
        let parsed = match parsed {
            Ok(parsed) => parsed,
            Err(error) => {
                let code = if error.starts_with("Unable to parse")
                    || error.starts_with("Unable to tokenize")
                {
                    "RustParseError"
                } else {
                    "RustScanError"
                };
                diagnostics.push(Diagnostic::new(code, error).at(path.clone(), Some(1)));
                continue;
            }
        };
        let mut visitor = Visitor {
            path,
            positions: SourceMap::new(&parsed.source),
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
    positions: SourceMap<'a>,
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
                            self.output.evidence.push(Evidence {
                                id: id.clone(),
                                location: self
                                    .positions
                                    .location(self.path, value.span().byte_range()),
                                scanner: "rust",
                            });
                            references.push(id);
                        }
                        Err(error) => {
                            let range = error.span().byte_range();
                            let location = if range.is_empty() {
                                SourceLocation::new(self.path, Some(1))
                            } else {
                                self.positions.location(self.path, range)
                            };
                            self.diagnostics.push(
                                Diagnostic::new(
                                    "RustReferenceError",
                                    format!(
                                        "Bloomery evidence tag must contain one string ID: {error}"
                                    ),
                                )
                                .at_location(location),
                            );
                        }
                    }
                }
            }
            self.output.tests.push(TestSite {
                scanner: "rust",
                name: Some(name),
                location: self
                    .positions
                    .location(self.path, item.sig.ident.span().byte_range()),
                references,
            });
        }
        visit::visit_item_fn(self, item);
    }
}
