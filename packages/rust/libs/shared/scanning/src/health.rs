//! Source-only Rust metrics used by the static code-health check.

use crate::AnalysisSession;
use bloomery_model::Diagnostic;
use proc_macro2::{Delimiter, Group, TokenStream, TokenTree};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthTarget {
    pub kind: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthSourceCrate {
    pub name: String,
    pub manifest_path: String,
    pub targets: Vec<HealthTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HealthToken {
    pub kind: String,
    pub text: String,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthCallable {
    pub identity: String,
    pub path: String,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub cyclomatic: usize,
    pub nesting: usize,
    pub body_tokens: Vec<HealthToken>,
    pub duplicate_token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueSource {
    pub path: String,
    pub line: usize,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthCrateMetrics {
    pub name: String,
    pub manifest_path: String,
    pub sources: Vec<String>,
    pub opaque_sources: Vec<OpaqueSource>,
    pub sloc: usize,
    pub callables: Vec<HealthCallable>,
    pub duplicate_tokens: usize,
    pub callable_tokens: usize,
}

/// Analyze all selected crates together so token clones can cross crate
/// boundaries while every physical source file is parsed at most once.
pub fn analyze_code_health(
    root: &Path,
    crates: &[HealthSourceCrate],
    duplicate_min_tokens: usize,
    session: &AnalysisSession,
) -> Result<Vec<HealthCrateMetrics>, Vec<Diagnostic>> {
    let root = match std::fs::canonicalize(root) {
        Ok(root) => root,
        Err(error) => {
            return Err(vec![Diagnostic::new(
                "HealthSourceError",
                format!("Unable to resolve workspace root: {error}"),
            )]);
        }
    };
    let mut output = crates
        .iter()
        .map(|crate_source| HealthCrateMetrics {
            name: crate_source.name.clone(),
            manifest_path: crate_source.manifest_path.clone(),
            sources: Vec::new(),
            opaque_sources: Vec::new(),
            sloc: 0,
            callables: Vec::new(),
            duplicate_tokens: 0,
            callable_tokens: 0,
        })
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    {
        let mut inputs = WalkInputs {
            root: &root,
            output: &mut output,
            session,
            diagnostics: &mut diagnostics,
        };
        for (crate_index, crate_source) in crates.iter().enumerate() {
            let mut visited = BTreeSet::new();
            for target in &crate_source.targets {
                let path = root
                    .join(&crate_source.manifest_path)
                    .parent()
                    .unwrap_or(&root)
                    .join(&target.path);
                let module_dir = path.parent().unwrap_or(&root).to_path_buf();
                walk_file(&mut inputs, crate_index, &path, &module_dir, &mut visited);
            }
        }
    }
    if !diagnostics.is_empty() {
        diagnostics.sort_by(|left, right| {
            left.location
                .as_ref()
                .map(|location| (&location.path, location.line))
                .cmp(
                    &right
                        .location
                        .as_ref()
                        .map(|location| (&location.path, location.line)),
                )
                .then(left.code.cmp(&right.code))
                .then(left.message.cmp(&right.message))
        });
        return Err(diagnostics);
    }
    mark_duplicates(&mut output, duplicate_min_tokens);
    for crate_metrics in &mut output {
        crate_metrics.sources.sort();
        crate_metrics.sources.dedup();
        crate_metrics.opaque_sources.sort_by(|left, right| {
            (&left.path, left.line, &left.description).cmp(&(
                &right.path,
                right.line,
                &right.description,
            ))
        });
        crate_metrics.opaque_sources.dedup();
        crate_metrics.callables.sort_by(|left, right| {
            (
                &left.path,
                left.start_line,
                left.start_column,
                &left.identity,
            )
                .cmp(&(
                    &right.path,
                    right.start_line,
                    right.start_column,
                    &right.identity,
                ))
        });
        crate_metrics.callable_tokens = crate_metrics
            .callables
            .iter()
            .map(|callable| callable.body_tokens.len())
            .sum();
        crate_metrics.duplicate_tokens = crate_metrics
            .callables
            .iter()
            .map(|callable| callable.duplicate_token_count)
            .sum();
    }
    Ok(output)
}

struct WalkInputs<'a> {
    root: &'a Path,
    output: &'a mut [HealthCrateMetrics],
    session: &'a AnalysisSession,
    diagnostics: &'a mut Vec<Diagnostic>,
}

fn walk_file(
    inputs: &mut WalkInputs<'_>,
    crate_index: usize,
    requested_path: &Path,
    module_dir: &Path,
    visited: &mut BTreeSet<PathBuf>,
) {
    let canonical = match std::fs::canonicalize(requested_path) {
        Ok(path) => path,
        Err(error) => {
            inputs.diagnostics.push(
                Diagnostic::new(
                    "HealthSourceError",
                    format!("Unable to resolve required Rust module: {error}"),
                )
                .at(requested_path.to_path_buf(), Some(1)),
            );
            return;
        }
    };
    if !canonical.starts_with(inputs.root) {
        inputs.diagnostics.push(
            Diagnostic::new(
                "HealthSourceEscape",
                "Rust health source resolves outside the repository",
            )
            .at(requested_path.to_path_buf(), Some(1)),
        );
        return;
    }
    if !visited.insert(canonical.clone()) {
        return;
    }
    let relative = canonical
        .strip_prefix(inputs.root)
        .unwrap_or(&canonical)
        .to_string_lossy()
        .replace('\\', "/");
    let parsed = match inputs.session.rust_file(&canonical) {
        Ok(parsed) => parsed,
        Err(error) => {
            inputs
                .diagnostics
                .push(Diagnostic::new("HealthSourceError", error).at(canonical, Some(1)));
            return;
        }
    };
    inputs.output[crate_index].sources.push(relative.clone());

    let mut file_walk = FileWalker {
        root: inputs.root,
        crate_index,
        source_path: &canonical,
        source_relative: &relative,
        module_dir: module_dir.to_path_buf(),
        tokens: &parsed.tokens,
        visited,
        output: inputs.output,
        session: inputs.session,
        diagnostics: inputs.diagnostics,
        module_stack: Vec::new(),
        callable_ordinal: 0,
        excluded_ranges: Vec::new(),
    };
    file_walk.walk_items(&parsed.syntax.items);
    let excluded = file_walk.excluded_ranges.clone();
    let sloc_lines = production_token_lines(&parsed.tokens, &excluded);
    file_walk.output[crate_index].sloc += sloc_lines.len();
}

#[derive(Clone, Copy)]
struct SourceRange {
    start: (usize, usize),
    end: (usize, usize),
}

impl SourceRange {
    fn contains(&self, position: (usize, usize)) -> bool {
        position >= self.start && position <= self.end
    }
}

struct FileWalker<'a> {
    root: &'a Path,
    crate_index: usize,
    source_path: &'a Path,
    source_relative: &'a str,
    module_dir: PathBuf,
    tokens: &'a TokenStream,
    visited: &'a mut BTreeSet<PathBuf>,
    output: &'a mut [HealthCrateMetrics],
    session: &'a AnalysisSession,
    diagnostics: &'a mut Vec<Diagnostic>,
    module_stack: Vec<String>,
    callable_ordinal: usize,
    excluded_ranges: Vec<SourceRange>,
}

impl FileWalker<'_> {
    fn walk_items(&mut self, items: &[syn::Item]) {
        for item in items {
            let attrs = item_attrs(item);
            if attrs.is_some_and(is_test_only) {
                self.exclude(item.span());
                continue;
            }
            match item {
                syn::Item::Fn(item) => {
                    if is_test_fn(&item.attrs) {
                        self.exclude(item.span());
                        continue;
                    }
                    self.add_callable(&item.sig.ident.to_string(), item.span(), &item.block);
                    self.discover_closures(&item.block);
                    self.inspect_macros(&item.block);
                }
                syn::Item::Mod(item) => {
                    self.module_stack.push(item.ident.to_string());
                    if let Some((_, items)) = &item.content {
                        let child = self.module_dir.join(item.ident.to_string());
                        let previous = std::mem::replace(&mut self.module_dir, child);
                        self.walk_items(items);
                        self.module_dir = previous;
                    } else {
                        for path in module_paths(item, &self.module_dir) {
                            let child_dir = module_child_dir(&path);
                            let mut inputs = WalkInputs {
                                root: self.root,
                                output: &mut *self.output,
                                session: self.session,
                                diagnostics: &mut *self.diagnostics,
                            };
                            walk_file(
                                &mut inputs,
                                self.crate_index,
                                &path,
                                &child_dir,
                                self.visited,
                            );
                        }
                    }
                    self.module_stack.pop();
                }
                syn::Item::Impl(item) => {
                    for member in &item.items {
                        if is_test_only(impl_item_attrs(member)) {
                            self.exclude(member.span());
                            continue;
                        }
                        if let syn::ImplItem::Fn(function) = member {
                            if is_test_only(&function.attrs) || is_test_fn(&function.attrs) {
                                self.exclude(function.span());
                                continue;
                            }
                            self.add_callable(
                                &function.sig.ident.to_string(),
                                function.span(),
                                &function.block,
                            );
                            self.discover_closures(&function.block);
                            self.inspect_macros(&function.block);
                        } else {
                            self.inspect_impl_member(member);
                        }
                    }
                }
                syn::Item::Trait(item) => {
                    for member in &item.items {
                        if let syn::TraitItem::Fn(function) = member {
                            if let Some(body) = &function.default {
                                if is_test_only(&function.attrs) {
                                    self.exclude(function.span());
                                    continue;
                                }
                                self.add_callable(
                                    &function.sig.ident.to_string(),
                                    function.span(),
                                    body,
                                );
                                self.discover_closures(body);
                                self.inspect_macros(body);
                            }
                        } else {
                            self.inspect_trait_member(member);
                        }
                    }
                }
                other => {
                    self.inspect_item(other);
                }
            }
        }
    }

    fn add_callable(&mut self, name: &str, span: proc_macro2::Span, body: &syn::Block) {
        let path = self.source_relative.to_owned();
        let identity_path = if self.module_stack.is_empty() {
            name.to_owned()
        } else {
            format!("{}::{name}", self.module_stack.join("::"))
        };
        let start = span.start();
        let end = span.end();
        self.callable_ordinal += 1;
        let identity = format!("{}#{:04}", identity_path, self.callable_ordinal);
        let mut metrics = BodyMetrics::new();
        metrics.visit_block(body);
        let open = body.brace_token.span.open().start();
        let close = body.brace_token.span.close().end();
        let mut nested_closures = ClosureBodyRanges::default();
        nested_closures.visit_block(body);
        let body_tokens = tokens_in_range(
            self.tokens,
            SourceRange {
                start: (open.line, open.column),
                end: (close.line, close.column),
            },
            &nested_closures.ranges,
        );
        self.output[self.crate_index]
            .callables
            .push(HealthCallable {
                identity,
                path,
                start_line: start.line,
                start_column: start.column,
                end_line: end.line,
                end_column: end.column,
                cyclomatic: metrics.cyclomatic,
                nesting: metrics.maximum_nesting,
                body_tokens,
                duplicate_token_count: 0,
            });
    }

    fn discover_closures(&mut self, node: &syn::Block) {
        let mut visitor = ClosureCollector {
            path: self.source_relative.to_owned(),
            module_path: self.module_stack.join("::"),
            tokens: self.tokens,
            callables: Vec::new(),
            ordinal: 0,
        };
        visitor.visit_block(node);
        self.output[self.crate_index]
            .callables
            .extend(visitor.callables);
    }

    fn inspect_macros(&mut self, node: &syn::Block) {
        let mut visitor = MacroCollector::default();
        visitor.visit_block(node);
        self.process_macros(visitor.macros);
    }

    fn inspect_impl_member(&mut self, member: &syn::ImplItem) {
        let mut visitor = MacroCollector::default();
        visitor.visit_impl_item(member);
        self.process_macros(visitor.macros);
    }

    fn inspect_trait_member(&mut self, member: &syn::TraitItem) {
        let mut visitor = MacroCollector::default();
        visitor.visit_trait_item(member);
        self.process_macros(visitor.macros);
        let mut closures = ClosureCollector {
            path: self.source_relative.to_owned(),
            module_path: self.module_stack.join("::"),
            tokens: self.tokens,
            callables: Vec::new(),
            ordinal: 0,
        };
        closures.visit_trait_item(member);
        self.output[self.crate_index]
            .callables
            .extend(closures.callables);
    }

    fn inspect_item(&mut self, item: &syn::Item) {
        let mut visitor = MacroCollector::default();
        visitor.visit_item(item);
        self.process_macros(visitor.macros);
        let mut closures = ClosureCollector {
            path: self.source_relative.to_owned(),
            module_path: self.module_stack.join("::"),
            tokens: self.tokens,
            callables: Vec::new(),
            ordinal: 0,
        };
        closures.visit_item(item);
        self.output[self.crate_index]
            .callables
            .extend(closures.callables);
    }

    fn process_macros(&mut self, macros: Vec<MacroReference>) {
        for reference in macros {
            if reference.name == "include" {
                if let Some(path) = reference.literal {
                    let include = self.source_path.parent().unwrap_or(self.root).join(path);
                    let module_dir = module_child_dir(&include);
                    let mut inputs = WalkInputs {
                        root: self.root,
                        output: &mut *self.output,
                        session: self.session,
                        diagnostics: &mut *self.diagnostics,
                    };
                    walk_file(
                        &mut inputs,
                        self.crate_index,
                        &include,
                        &module_dir,
                        self.visited,
                    );
                } else {
                    self.output[self.crate_index]
                        .opaque_sources
                        .push(OpaqueSource {
                            path: self.source_relative.to_owned(),
                            line: reference.line,
                            description: "nonliteral include! path".to_owned(),
                        });
                }
            } else {
                self.output[self.crate_index]
                    .opaque_sources
                    .push(OpaqueSource {
                        path: self.source_relative.to_owned(),
                        line: reference.line,
                        description: format!("macro invocation {}!", reference.name),
                    });
            }
        }
    }
    fn exclude(&mut self, span: proc_macro2::Span) {
        let start = span.start();
        let end = span.end();
        self.excluded_ranges.push(SourceRange {
            start: (start.line, start.column),
            end: (end.line, end.column),
        });
    }
}

#[derive(Default)]
struct BodyMetrics {
    cyclomatic: usize,
    current_nesting: usize,
    maximum_nesting: usize,
}

impl<'ast> Visit<'ast> for BodyMetrics {
    fn visit_expr_closure(&mut self, _node: &'ast syn::ExprClosure) {}

    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        self.cyclomatic += 1;
        self.visit_expr(&node.cond);
        self.enter_nesting();
        self.visit_block(&node.then_branch);
        self.leave_nesting();
        if let Some((_, otherwise)) = &node.else_branch {
            if let syn::Expr::If(other_if) = otherwise.as_ref() {
                self.visit_expr_if(other_if);
            } else {
                self.enter_nesting();
                self.visit_expr(otherwise);
                self.leave_nesting();
            }
        }
    }

    fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
        self.cyclomatic += node.arms.len().saturating_sub(1);
        for arm in &node.arms {
            if arm.guard.is_some() {
                self.cyclomatic += 1;
            }
        }
        self.visit_expr(&node.expr);
        self.enter_nesting();
        for arm in &node.arms {
            self.visit_arm(arm);
        }
        self.leave_nesting();
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.cyclomatic += 1;
        self.visit_expr(&node.expr);
        self.enter_nesting();
        self.visit_block(&node.body);
        self.leave_nesting();
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.cyclomatic += 1;
        self.visit_expr(&node.cond);
        self.enter_nesting();
        self.visit_block(&node.body);
        self.leave_nesting();
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        self.cyclomatic += 1;
        self.enter_nesting();
        self.visit_block(&node.body);
        self.leave_nesting();
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if matches!(node.op, syn::BinOp::And(_) | syn::BinOp::Or(_)) {
            self.cyclomatic += 1;
        }
        visit::visit_expr_binary(self, node);
    }

    fn visit_expr_try(&mut self, node: &'ast syn::ExprTry) {
        self.cyclomatic += 1;
        visit::visit_expr_try(self, node);
    }
}

impl BodyMetrics {
    fn new() -> Self {
        Self {
            cyclomatic: 1,
            current_nesting: 0,
            maximum_nesting: 0,
        }
    }

    fn enter_nesting(&mut self) {
        self.current_nesting += 1;
        self.maximum_nesting = self.maximum_nesting.max(self.current_nesting);
    }
    fn leave_nesting(&mut self) {
        self.current_nesting = self.current_nesting.saturating_sub(1);
    }
}

struct ClosureCollector<'a> {
    path: String,
    module_path: String,
    tokens: &'a TokenStream,
    callables: Vec<HealthCallable>,
    ordinal: usize,
}

impl<'ast> Visit<'ast> for ClosureCollector<'_> {
    fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
        let mut metrics = BodyMetrics::new();
        metrics.visit_expr(&closure.body);
        let span = closure.span();
        let start = span.start();
        let end = span.end();
        self.ordinal += 1;
        let local = format!("closure@{}:{}#{}", start.line, start.column, self.ordinal);
        let identity = if self.module_path.is_empty() {
            local
        } else {
            format!("{}::{local}", self.module_path)
        };
        self.callables.push(HealthCallable {
            identity,
            path: self.path.clone(),
            start_line: start.line,
            start_column: start.column,
            end_line: end.line,
            end_column: end.column,
            cyclomatic: metrics.cyclomatic,
            nesting: metrics.maximum_nesting,
            body_tokens: {
                let mut nested_closures = ClosureBodyRanges::default();
                nested_closures.visit_expr(&closure.body);
                tokens_in_range(
                    self.tokens,
                    span_range(closure.body.span()),
                    &nested_closures.ranges,
                )
            },
            duplicate_token_count: 0,
        });
        visit::visit_expr(self, &closure.body);
    }
}

#[derive(Default)]
struct ClosureBodyRanges {
    ranges: Vec<SourceRange>,
}

impl<'ast> Visit<'ast> for ClosureBodyRanges {
    fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
        self.ranges.push(span_range(closure.body.span()));
        visit::visit_expr(self, &closure.body);
    }
}

#[derive(Default)]
struct MacroCollector {
    macros: Vec<MacroReference>,
}

struct MacroReference {
    name: String,
    literal: Option<PathBuf>,
    line: usize,
}

impl<'ast> Visit<'ast> for MacroCollector {
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        let name = mac
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .unwrap_or_else(|| "<macro>".to_owned());
        let literal = (name == "include")
            .then(|| syn::parse2::<syn::LitStr>(mac.tokens.clone()).ok())
            .flatten()
            .map(|literal| PathBuf::from(literal.value()));
        self.macros.push(MacroReference {
            name,
            literal,
            line: mac.span().start().line,
        });
    }
}

fn span_range(span: proc_macro2::Span) -> SourceRange {
    let start = span.start();
    let end = span.end();
    SourceRange {
        start: (start.line, start.column),
        end: (end.line, end.column),
    }
}

fn item_attrs(item: &syn::Item) -> Option<&[syn::Attribute]> {
    Some(match item {
        syn::Item::Const(value) => &value.attrs,
        syn::Item::Enum(value) => &value.attrs,
        syn::Item::ExternCrate(value) => &value.attrs,
        syn::Item::Fn(value) => &value.attrs,
        syn::Item::ForeignMod(value) => &value.attrs,
        syn::Item::Impl(value) => &value.attrs,
        syn::Item::Macro(value) => &value.attrs,
        syn::Item::Mod(value) => &value.attrs,
        syn::Item::Static(value) => &value.attrs,
        syn::Item::Struct(value) => &value.attrs,
        syn::Item::Trait(value) => &value.attrs,
        syn::Item::TraitAlias(value) => &value.attrs,
        syn::Item::Type(value) => &value.attrs,
        syn::Item::Union(value) => &value.attrs,
        syn::Item::Use(value) => &value.attrs,
        _ => return None,
    })
}

fn impl_item_attrs(item: &syn::ImplItem) -> &[syn::Attribute] {
    match item {
        syn::ImplItem::Const(value) => &value.attrs,
        syn::ImplItem::Fn(value) => &value.attrs,
        syn::ImplItem::Type(value) => &value.attrs,
        syn::ImplItem::Macro(value) => &value.attrs,
        _ => &[],
    }
}

fn is_test_only(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("cfg")
            && attr
                .parse_args::<syn::Meta>()
                .is_ok_and(|meta| matches!(meta, syn::Meta::Path(path) if path.is_ident("test")))
    })
}

fn is_test_fn(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("test")
            || attr
                .path()
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "test")
    })
}

fn module_paths(module: &syn::ItemMod, module_dir: &Path) -> Vec<PathBuf> {
    let explicit = module
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("path"))
        .filter_map(|attr| match &attr.meta {
            syn::Meta::NameValue(value) => match &value.value {
                syn::Expr::Lit(expression) => match &expression.lit {
                    syn::Lit::Str(path) => Some(path.value()),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        })
        .map(|path| module_dir.join(path))
        .collect::<Vec<_>>();
    if !explicit.is_empty() {
        return explicit;
    }
    let name = module.ident.to_string();
    let first = module_dir.join(format!("{name}.rs"));
    let second = module_dir.join(name).join("mod.rs");
    let paths = [first.clone(), second]
        .into_iter()
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    if paths.is_empty() { vec![first] } else { paths }
}

fn module_child_dir(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or(Path::new("."));
    if path.file_name().is_some_and(|name| name == "mod.rs") {
        parent.to_path_buf()
    } else {
        parent.join(path.file_stem().unwrap_or_default())
    }
}

fn production_token_lines(tokens: &TokenStream, excluded: &[SourceRange]) -> BTreeSet<usize> {
    let mut lines = BTreeSet::new();
    flatten_tokens(tokens, &mut |token| {
        let start = (token.start_line, token.start_column);
        let end = (token.end_line, token.end_column);
        let is_excluded = excluded
            .iter()
            .any(|range| range.contains(start) && range.contains(end));
        if is_excluded {
            return;
        }
        lines.insert(token.start_line);
        if token.end_line != token.start_line {
            for line in token.start_line.saturating_add(1)..=token.end_line {
                lines.insert(line);
            }
        }
    });
    lines
}

fn tokens_in_range(
    tokens: &TokenStream,
    range: SourceRange,
    excluded: &[SourceRange],
) -> Vec<HealthToken> {
    let mut selected = Vec::new();
    flatten_tokens(tokens, &mut |token| {
        let start = (token.start_line, token.start_column);
        let end = (token.end_line, token.end_column);
        if position_in_range(start, range)
            && position_in_range(end, range)
            && !excluded
                .iter()
                .any(|nested| position_in_range(start, *nested) && position_in_range(end, *nested))
        {
            selected.push(token);
        }
    });
    selected
}

fn position_in_range(position: (usize, usize), range: SourceRange) -> bool {
    position >= range.start && position <= range.end
}

fn flatten_tokens(tokens: &TokenStream, visitor: &mut impl FnMut(HealthToken)) {
    for token in tokens.clone() {
        match token {
            TokenTree::Group(group) => flatten_group(&group, visitor),
            TokenTree::Ident(ident) => {
                let span = ident.span();
                visitor(HealthToken {
                    kind: "ident".to_owned(),
                    text: ident.to_string(),
                    start_line: span.start().line,
                    start_column: span.start().column,
                    end_line: span.end().line,
                    end_column: span.end().column,
                });
            }
            TokenTree::Punct(punct) => {
                let span = punct.span();
                visitor(HealthToken {
                    kind: format!("punct:{:?}", punct.spacing()),
                    text: punct.as_char().to_string(),
                    start_line: span.start().line,
                    start_column: span.start().column,
                    end_line: span.end().line,
                    end_column: span.end().column,
                });
            }
            TokenTree::Literal(literal) => {
                let span = literal.span();
                visitor(HealthToken {
                    kind: "literal".to_owned(),
                    text: literal.to_string(),
                    start_line: span.start().line,
                    start_column: span.start().column,
                    end_line: span.end().line,
                    end_column: span.end().column,
                });
            }
        }
    }
}

fn flatten_group(group: &Group, visitor: &mut impl FnMut(HealthToken)) {
    let span = group.span();
    let delimiter = match group.delimiter() {
        Delimiter::Parenthesis => "paren",
        Delimiter::Brace => "brace",
        Delimiter::Bracket => "bracket",
        Delimiter::None => "none",
    };
    if group.delimiter() != Delimiter::None {
        let start = span.start();
        visitor(HealthToken {
            kind: format!("open:{delimiter}"),
            text: format!("{delimiter}:open"),
            start_line: start.line,
            start_column: start.column,
            end_line: start.line,
            end_column: start.column,
        });
    }
    flatten_tokens(&group.stream(), visitor);
    if group.delimiter() != Delimiter::None {
        let end = span.end();
        visitor(HealthToken {
            kind: format!("close:{delimiter}"),
            text: format!("{delimiter}:close"),
            start_line: end.line,
            start_column: end.column,
            end_line: end.line,
            end_column: end.column,
        });
    }
}

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct TokenKey(String, String);

#[derive(Clone)]
struct WindowRef {
    crate_index: usize,
    callable_index: usize,
    start: usize,
    end: usize,
    physical_callable: (String, usize, usize, usize, usize),
}

fn mark_duplicates(crates: &mut [HealthCrateMetrics], minimum: usize) {
    if minimum == 0 {
        return;
    }
    let mut groups: HashMap<u64, Vec<(Vec<TokenKey>, WindowRef)>> = HashMap::new();
    let mut covered = crates
        .iter()
        .map(|crate_metrics| {
            crate_metrics
                .callables
                .iter()
                .map(|callable| vec![false; callable.body_tokens.len()])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for (crate_index, crate_metrics) in crates.iter().enumerate() {
        for (callable_index, callable) in crate_metrics.callables.iter().enumerate() {
            if callable.body_tokens.len() < minimum {
                continue;
            }
            let physical_callable = (
                callable.path.clone(),
                callable.start_line,
                callable.start_column,
                callable.end_line,
                callable.end_column,
            );
            for start in 0..=callable.body_tokens.len() - minimum {
                let end = start + minimum;
                let key = callable.body_tokens[start..end]
                    .iter()
                    .map(|token| TokenKey(token.kind.clone(), token.text.clone()))
                    .collect::<Vec<_>>();
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                key.hash(&mut hasher);
                groups.entry(hasher.finish()).or_default().push((
                    key,
                    WindowRef {
                        crate_index,
                        callable_index,
                        start,
                        end,
                        physical_callable: physical_callable.clone(),
                    },
                ));
            }
        }
    }
    for candidates in groups.values() {
        let mut exact: BTreeMap<Vec<TokenKey>, Vec<&WindowRef>> = BTreeMap::new();
        for (key, reference) in candidates {
            exact.entry(key.clone()).or_default().push(reference);
        }
        for references in exact.values() {
            let distinct = references
                .iter()
                .map(|reference| reference.physical_callable.clone())
                .collect::<BTreeSet<_>>();
            if distinct.len() < 2 {
                continue;
            }
            for reference in references {
                covered[reference.crate_index][reference.callable_index]
                    [reference.start..reference.end]
                    .fill(true);
            }
        }
    }
    for (crate_index, crate_metrics) in crates.iter_mut().enumerate() {
        for (callable_index, callable) in crate_metrics.callables.iter_mut().enumerate() {
            callable.duplicate_token_count = covered[crate_index][callable_index]
                .iter()
                .filter(|covered| **covered)
                .count();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HealthSourceCrate, HealthTarget, analyze_code_health};
    use crate::AnalysisSession;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("bloomery-health-{stamp}"))
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-003"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-004"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-005"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-014"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-015"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-016"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-017"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-018"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-019"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-020"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-021"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-023"))]
    fn source_health_resolves_modules_and_measures_callable_bodies() {
        let root = temp_root();
        let source_dir = root.join("crates/demo/src");
        fs::create_dir_all(&source_dir).expect("source directory");
        let source = r#"
pub fn outer(flag: bool) {
    if flag && true {
        let nested = || { if flag { 1 } else { 0 } };
        let _ = nested();
    }
}

#[path = "alt.rs"]
mod alternative;

include!("included.rs");

fn clone_a() { let value = 1; if value > 0 { let result = value + 2; drop(result); } }
fn clone_b() { let value = 1; if value > 0 { let result = value + 2; drop(result); } }

#[cfg(test)]
mod tests {
    #[test]
    fn hidden() { panic!("test only"); }
}
"#;
        fs::write(source_dir.join("lib.rs"), source).expect("lib source");
        fs::write(
            source_dir.join("alt.rs"),
            "pub fn alternative() { let _ = 1; }\n",
        )
        .expect("module source");
        fs::write(
            source_dir.join("included.rs"),
            "pub fn included() { let _ = 2; }\n",
        )
        .expect("included source");
        let crates = [HealthSourceCrate {
            name: "demo".to_owned(),
            manifest_path: "crates/demo/Cargo.toml".to_owned(),
            targets: vec![HealthTarget {
                kind: "library".to_owned(),
                path: "src/lib.rs".to_owned(),
            }],
        }];
        let report = analyze_code_health(&root, &crates, 16, &AnalysisSession::new())
            .expect("health analysis");
        let crate_report = &report[0];
        assert!(
            crate_report
                .sources
                .contains(&"crates/demo/src/alt.rs".to_owned())
        );
        assert!(
            crate_report
                .sources
                .contains(&"crates/demo/src/included.rs".to_owned())
        );
        assert!(
            crate_report
                .callables
                .iter()
                .all(|callable| !callable.identity.contains("hidden"))
        );
        let outer = crate_report
            .callables
            .iter()
            .find(|callable| callable.identity.starts_with("outer#"))
            .expect("outer callable");
        assert_eq!(outer.cyclomatic, 3);
        assert_eq!(outer.nesting, 1);
        assert_eq!(outer.path, "crates/demo/src/lib.rs");
        assert!(outer.start_line > 0 && outer.end_line >= outer.start_line);
        let closure = crate_report
            .callables
            .iter()
            .find(|callable| callable.identity.contains("closure@"))
            .expect("closure callable");
        assert_eq!(closure.cyclomatic, 2);
        assert_eq!(closure.nesting, 1);
        assert!(outer.body_tokens.len() < source.matches(" ").count());
        assert!(crate_report.sloc > 0);
        assert!(crate_report.callable_tokens >= crate_report.duplicate_tokens);
        assert!(crate_report.duplicate_tokens > 0);
        assert!(
            crate_report
                .opaque_sources
                .iter()
                .all(|item| !item.description.contains("include"))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-003"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-012"))]
    fn required_missing_modules_are_diagnostics() {
        let root = temp_root();
        let source_dir = root.join("crates/demo/src");
        fs::create_dir_all(&source_dir).expect("source directory");
        fs::write(source_dir.join("lib.rs"), "mod missing;\n").expect("source");
        let crates = [HealthSourceCrate {
            name: "demo".to_owned(),
            manifest_path: "crates/demo/Cargo.toml".to_owned(),
            targets: vec![HealthTarget {
                kind: "library".to_owned(),
                path: "src/lib.rs".to_owned(),
            }],
        }];
        let diagnostics = analyze_code_health(&root, &crates, 16, &AnalysisSession::new())
            .expect_err("missing module");
        assert_eq!(diagnostics[0].code, "HealthSourceError");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-022"))]
    fn duplicate_token_coverage_is_attributed_to_each_crate() {
        let root = temp_root();
        for crate_name in ["first", "second"] {
            let source_dir = root.join(format!("crates/{crate_name}/src"));
            fs::create_dir_all(&source_dir).expect("source directory");
            fs::write(
                source_dir.join("lib.rs"),
                "pub fn repeated() { let item = 1; if item > 0 { drop(item); } }\n",
            )
            .expect("crate source");
        }
        let crates = ["first", "second"].map(|crate_name| HealthSourceCrate {
            name: crate_name.to_owned(),
            manifest_path: format!("crates/{crate_name}/Cargo.toml"),
            targets: vec![HealthTarget {
                kind: "library".to_owned(),
                path: "src/lib.rs".to_owned(),
            }],
        });
        let report = analyze_code_health(&root, &crates, 16, &AnalysisSession::new())
            .expect("health analysis");
        assert_eq!(report.len(), 2);
        assert!(report.iter().all(|crate_report| {
            crate_report.duplicate_tokens > 0
                && crate_report.duplicate_tokens <= crate_report.callable_tokens
        }));
        let _ = fs::remove_dir_all(root);
    }
}
