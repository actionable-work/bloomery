use crate::ScanOutput;
use crate::SourceTree;
use crate::analysis::AnalysisSession;
use crate::conditions::{self, CfgExpression, RustCondition};
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
    collect_conditions: bool,
) -> Result<(ScanOutput, Vec<RustCondition>), Vec<Diagnostic>> {
    let mut output = ScanOutput::default();
    let mut diagnostics = Vec::new();
    let mut conditions = Vec::new();
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
            collect_conditions,
            inherited: conditions::attributes(&parsed.syntax.attrs),
            conditions: Vec::new(),
            positions: SourceMap::new(&parsed.source),
            output: ScanOutput::default(),
            diagnostics: Vec::new(),
        };
        visitor.visit_file(&parsed.syntax);
        output.absorb(visitor.output);
        conditions.extend(visitor.conditions);
        diagnostics.extend(visitor.diagnostics);
    }
    if diagnostics.is_empty() {
        Ok((output, conditions))
    } else {
        Err(diagnostics)
    }
}

struct Visitor<'a> {
    path: &'a Path,
    positions: SourceMap<'a>,
    collect_conditions: bool,
    inherited: CfgExpression,
    conditions: Vec<RustCondition>,
    output: ScanOutput,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for Visitor<'_> {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        let inherited = self.inherited.clone();
        self.inherited =
            CfgExpression::All(vec![inherited.clone(), conditions::attributes(&item.attrs)]);
        visit::visit_item_mod(self, item);
        self.inherited = inherited;
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let inherited = self.inherited.clone();
        let attributes = conditions::attributes(&item.attrs);
        let conditional_test =
            self.collect_conditions && conditions::conditional_attribute(&attributes, "test");
        self.inherited = CfgExpression::All(vec![inherited.clone(), attributes]);
        let is_test = item.attrs.iter().any(|attribute| {
            attribute
                .path()
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "test")
        });
        if is_test || conditional_test {
            let evidence_start = self.output.evidence.len();
            let name = item.sig.ident.to_string();
            let references = self.references(&item.attrs);
            let location = self
                .positions
                .location(self.path, item.sig.ident.span().byte_range());
            if self.collect_conditions {
                self.conditions.push(RustCondition {
                    test: location.clone(),
                    references: self.output.evidence[evidence_start..]
                        .iter()
                        .map(|evidence| evidence.location.clone())
                        .collect(),
                    expression: CfgExpression::All(vec![
                        self.inherited.clone(),
                        if is_test {
                            CfgExpression::All(Vec::new())
                        } else {
                            CfgExpression::Unsupported("conditional_test_attribute".into())
                        },
                    ]),
                });
            }
            self.output.tests.push(TestSite {
                scanner: "rust",
                name: Some(name),
                location,
                references,
            });
        }
        visit::visit_item_fn(self, item);
        self.inherited = inherited;
    }
}

impl Visitor<'_> {
    fn references(&mut self, attrs: &[syn::Attribute]) -> Vec<String> {
        let mut references = Vec::new();
        let tags = attrs
            .iter()
            .filter(|attr| attr.path().is_ident("cfg_attr"))
            .filter_map(|attr| {
                attr.parse_args_with(Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated)
                    .ok()
            })
            .flatten()
            .filter_map(|meta| match meta {
                syn::Meta::List(list) if list.path.is_ident("bloomery") => Some(list),
                _ => None,
            });
        for list in tags {
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
                            format!("Bloomery evidence tag must contain one string ID: {error}"),
                        )
                        .at_location(location),
                    );
                }
            }
        }
        references
    }
}
