use crate::ScanOutput;
use crate::files::expand_globs;
use bloomery_model::{Diagnostic, Evidence, SourceLocation, TestSite};
use std::fs;
use std::path::Path;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(test)]
pub(crate) static PARSE_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn scan(root: &Path, patterns: &[String]) -> Result<ScanOutput, Vec<Diagnostic>> {
    let paths = match expand_globs(root, patterns) {
        Ok(paths) => paths,
        Err(error) => return Err(vec![Diagnostic::new("ScannerError", error)]),
    };
    let mut output = ScanOutput::default();
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
        let syntax = match parse(&contents) {
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
            output: ScanOutput::default(),
            diagnostics: Vec::new(),
        };
        visitor.visit_file(&syntax);
        output.absorb(visitor.output);
        diagnostics.extend(visitor.diagnostics);
    }
    if diagnostics.is_empty() {
        Ok(output)
    } else {
        Err(diagnostics)
    }
}

fn parse(contents: &str) -> syn::Result<syn::File> {
    #[cfg(test)]
    PARSE_COUNT.fetch_add(1, Ordering::Relaxed);
    syn::parse_file(contents)
}

struct Visitor<'a> {
    path: &'a Path,
    contents: &'a str,
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
                            let line = line_for(&format!("bloomery(\"{id}\""), self.contents);
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
            let line = line_for(&format!("fn {name}"), self.contents);
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

fn line_for(needle: &str, contents: &str) -> usize {
    contents
        .find(needle)
        .map(|offset| {
            contents[..offset]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
                + 1
        })
        .unwrap_or(1)
}
