use crate::{AnalysisSession, CfgExpression, conditions, positions::SourceMap};
use bloomery_model::{Diagnostic, SourceLocation};
use bloomery_workspace::source::{SourceTree, TreeIndex};
use std::path::{Path, PathBuf};

/// Direct out-of-line declarations; candidates have not been normalized/resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustModule {
    pub name: String,
    pub location: SourceLocation,
    pub candidates: Vec<PathBuf>,
    pub condition: CfgExpression,
    pub unsupported_path: bool,
}

pub(crate) fn root_modules(
    tree: &dyn SourceTree,
    root: &str,
    session: &AnalysisSession,
) -> Result<Vec<RustModule>, Vec<Diagnostic>> {
    let project = || -> Result<Vec<RustModule>, String> {
        let index = TreeIndex::new(tree)?;
        let path = Path::new(root);
        if !index.files().any(|file| file == root) {
            return Err("Rust root is not a canonical indexed file".into());
        }
        let parsed = session.rust_source_file(path, || tree.read(root))?;
        let positions = SourceMap::new(&parsed.source);
        let inherited = conditions::attributes(&parsed.syntax.attrs);
        Ok(parsed
            .syntax
            .items
            .iter()
            .filter_map(|item| {
                let syn::Item::Mod(module) = item else {
                    return None;
                };
                if module.content.is_some() {
                    return None;
                }
                let attributes = conditions::attributes(&module.attrs);
                let candidates =
                    module_paths(module, path.parent().unwrap_or(Path::new("")), |_| true);
                let explicit = module
                    .attrs
                    .iter()
                    .filter(|attr| attr.path().is_ident("path"))
                    .count();
                let unsupported_path = explicit > 1
                    || (explicit > 0 && explicit != candidates.len())
                    || conditions::conditional_attribute(&attributes, "path");
                Some(RustModule {
                    name: module.ident.to_string(),
                    location: positions.location(path, module.ident.span().byte_range()),
                    candidates,
                    condition: CfgExpression::All(vec![inherited.clone(), attributes]),
                    unsupported_path,
                })
            })
            .collect())
    };
    project().map_err(|error| vec![Diagnostic::new("RustModuleRootError", error).at(root, None)])
}

pub(crate) fn module_paths(
    module: &syn::ItemMod,
    module_dir: &Path,
    exists: impl Fn(&Path) -> bool,
) -> Vec<PathBuf> {
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
        .filter(|path| exists(path))
        .collect::<Vec<_>>();
    if paths.is_empty() { vec![first] } else { paths }
}
