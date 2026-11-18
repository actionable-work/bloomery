//! Workspace dependency declarations that no target source references.
//!
//! A dependency counts as used when its Rust crate name appears as an
//! identifier token anywhere in the module tree of a target that can see it,
//! including attributes, macro bodies, and conditionally compiled code.

mod manifest;

use bloomery_cargo_metadata::{Dependency, DependencyKind, LIBRARY_KINDS, Metadata, Package};
use bloomery_model::{Diagnostic, sort_diagnostics};
use bloomery_scanning::{AnalysisSession, module_tree_files};
use proc_macro2::{TokenStream, TokenTree};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const UNUSED_CODE: &str = "UnusedDependency";
const SOURCE_ERROR_CODE: &str = "UnusedDependencySourceError";

/// Report every workspace member dependency that its visible target sources
/// never reference. `exceptions` lists dependency package names to skip.
pub fn check_workspace_unused_dependencies(
    root: &Path,
    exceptions: &[String],
) -> Result<(), Vec<Diagnostic>> {
    let metadata = Metadata::load(root).map_err(source_error)?;
    let root = std::fs::canonicalize(root)
        .map_err(|error| source_error(format!("Unable to resolve repository root: {error}")))?;
    let exceptions = exceptions
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let session = AnalysisSession::new();
    let mut diagnostics = Vec::new();
    for package in metadata.workspace_packages() {
        let inputs = PackageInputs {
            root: &root,
            metadata: &metadata,
            package,
            exceptions: &exceptions,
            session: &session,
        };
        diagnostics.extend(check_package(&inputs));
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        sort_diagnostics(&mut diagnostics);
        Err(diagnostics)
    }
}

struct PackageInputs<'a> {
    root: &'a Path,
    metadata: &'a Metadata,
    package: &'a Package,
    exceptions: &'a BTreeSet<&'a str>,
    session: &'a AnalysisSession,
}

fn check_package(inputs: &PackageInputs<'_>) -> Vec<Diagnostic> {
    let usage = match package_usage(inputs.root, inputs.package, inputs.session) {
        Ok(usage) => usage,
        // Incomplete sources would make every dependency look unused.
        Err(diagnostics) => return diagnostics,
    };
    let manifest_path = relative(inputs.root, &inputs.package.manifest_path);
    let manifest = std::fs::read_to_string(&inputs.package.manifest_path).unwrap_or_default();
    inputs
        .package
        .dependencies
        .iter()
        .filter(|dependency| !inputs.exceptions.contains(dependency.name.as_str()))
        .filter_map(|dependency| {
            let crate_name = extern_crate_name(inputs.metadata, dependency);
            let referenced = usage
                .get(&dependency.kind())
                .is_some_and(|identifiers| identifiers.contains(&crate_name));
            (!referenced).then(|| {
                unused_diagnostic(&inputs.package.name, dependency, &crate_name).at(
                    manifest_path.clone(),
                    manifest::declaration_line(&manifest, dependency),
                )
            })
        })
        .collect()
}

/// Identifiers referenced by the targets visible to each dependency kind.
fn package_usage(
    root: &Path,
    package: &Package,
    session: &AnalysisSession,
) -> Result<BTreeMap<DependencyKind, BTreeSet<String>>, Vec<Diagnostic>> {
    let mut usage = BTreeMap::<DependencyKind, BTreeSet<String>>::new();
    let mut errors = Vec::new();
    for target in &package.targets {
        let identifiers = match target_identifiers(&target.src_path, session) {
            Ok(identifiers) => identifiers,
            Err(failures) => {
                errors.extend(failures.into_iter().map(|(path, message)| {
                    Diagnostic::new(SOURCE_ERROR_CODE, message).at(relative(root, &path), None)
                }));
                continue;
            }
        };
        for kind in visible_kinds(target) {
            usage
                .entry(kind)
                .or_default()
                .extend(identifiers.iter().cloned());
        }
    }
    if errors.is_empty() {
        Ok(usage)
    } else {
        Err(errors)
    }
}

/// Dependency kinds whose declarations a target's sources can reference.
fn visible_kinds(target: &bloomery_cargo_metadata::Target) -> Vec<DependencyKind> {
    if target.has_kind(&["custom-build"]) {
        vec![DependencyKind::Build]
    } else if target.has_kind(&LIBRARY_KINDS) || target.has_kind(&["bin"]) {
        vec![DependencyKind::Normal, DependencyKind::Development]
    } else {
        vec![DependencyKind::Development]
    }
}

fn target_identifiers(
    entry: &Path,
    session: &AnalysisSession,
) -> Result<BTreeSet<String>, Vec<(PathBuf, String)>> {
    let mut identifiers = BTreeSet::new();
    let mut failures = Vec::new();
    for file in module_tree_files(entry, session, &mut BTreeSet::new()) {
        match file {
            Ok(parsed) => collect_identifiers(&parsed.tokens, &mut identifiers),
            Err(error) => failures.push((error.path, error.message)),
        }
    }
    if failures.is_empty() {
        Ok(identifiers)
    } else {
        Err(failures)
    }
}

fn collect_identifiers(tokens: &TokenStream, identifiers: &mut BTreeSet<String>) {
    let mut pending = vec![tokens.clone()];
    while let Some(stream) = pending.pop() {
        for tree in stream {
            match tree {
                TokenTree::Group(group) => pending.push(group.stream()),
                TokenTree::Ident(ident) => {
                    let name = ident.to_string();
                    let name = name.strip_prefix("r#").unwrap_or(&name).to_owned();
                    identifiers.insert(name);
                }
                TokenTree::Punct(_) | TokenTree::Literal(_) => {}
            }
        }
    }
}

/// Name the dependency is bound to in Rust source.
fn extern_crate_name(metadata: &Metadata, dependency: &Dependency) -> String {
    let name = match &dependency.rename {
        Some(rename) => rename.as_str(),
        None => metadata
            .library_crate_name(&dependency.name)
            .unwrap_or(&dependency.name),
    };
    name.replace('-', "_")
}

fn unused_diagnostic(package: &str, dependency: &Dependency, crate_name: &str) -> Diagnostic {
    let (section, scope) = match dependency.kind() {
        DependencyKind::Normal => ("dependencies", "library, binary, or proc-macro"),
        DependencyKind::Development => ("dev-dependencies", "target"),
        DependencyKind::Build => ("build-dependencies", "build-script"),
    };
    Diagnostic::new(
        UNUSED_CODE,
        format!(
            "{package} declares unused {section} entry '{}'",
            dependency.manifest_key()
        ),
    )
    .note(format!(
        "No {scope} source of {package} references `{crate_name}`; remove the entry, move it to the section whose targets use it, or list '{}' in checks.unusedDependencies.exceptions",
        dependency.name
    ))
}

fn relative(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

fn source_error(message: String) -> Vec<Diagnostic> {
    vec![Diagnostic::new(SOURCE_ERROR_CODE, message)]
}

#[cfg(test)]
mod tests;
