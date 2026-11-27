//! Workspace dependency declarations that no target source references.
//!
//! A dependency counts as used when its Rust crate name starts a path, a `use`
//! tree, or an `extern crate` item anywhere in the module tree of a target that
//! can see it, including attributes, macro bodies, and conditionally compiled
//! code. Normal dependencies ignore code confined to `cfg(test)`.

mod cfg_test;
mod doc_text;
mod manifest;
mod references;

use bloomery_cargo_metadata::{Dependency, DependencyKind, LIBRARY_KINDS, Metadata, Package};
use bloomery_model::{Diagnostic, sort_diagnostics};
use bloomery_scanning::{AnalysisSession, module_tree_scopes};
use references::References;
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
        for (kind, scope) in visible_scopes(target) {
            let names = match scope {
                Scope::Production => &identifiers.production,
                Scope::All => &identifiers.all,
            };
            usage.entry(kind).or_default().extend(names.iter().cloned());
        }
    }
    if errors.is_empty() {
        Ok(usage)
    } else {
        Err(errors)
    }
}

/// Which of a target's references count toward a dependency kind.
#[derive(Clone, Copy)]
enum Scope {
    /// References outside code confined to `cfg(test)`.
    Production,
    /// Every reference, including test-only code.
    All,
}

/// Dependency kinds whose declarations a target's sources can reference.
fn visible_scopes(target: &bloomery_cargo_metadata::Target) -> Vec<(DependencyKind, Scope)> {
    if target.has_kind(&["custom-build"]) {
        vec![(DependencyKind::Build, Scope::All)]
    } else if target.has_kind(&LIBRARY_KINDS) || target.has_kind(&["bin"]) {
        vec![
            (DependencyKind::Normal, Scope::Production),
            (DependencyKind::Development, Scope::All),
        ]
    } else {
        vec![(DependencyKind::Development, Scope::All)]
    }
}

fn target_identifiers(
    entry: &Path,
    session: &AnalysisSession,
) -> Result<References, Vec<(PathBuf, String)>> {
    let mut identifiers = References::default();
    let mut failures = Vec::new();
    let confines =
        |predicate: &proc_macro2::TokenStream| cfg_test::predicate_requires_test(predicate);
    for file in module_tree_scopes(entry, session, &mut BTreeSet::new(), &confines) {
        match file {
            Ok((parsed, test_only)) => identifiers.collect(&parsed.tokens, test_only),
            Err(error) => failures.push((error.path, error.message)),
        }
    }
    if failures.is_empty() {
        Ok(identifiers)
    } else {
        Err(failures)
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
        DependencyKind::Normal => ("dependencies", "non-test library, binary, or proc-macro"),
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
        "No {scope} source of {package} references `{crate_name}`; remove the entry, move it to the section whose targets use it, add `use {crate_name} as _;` to a target that needs it only for linking, or list '{}' in checks.unusedDependencies.exceptions",
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
