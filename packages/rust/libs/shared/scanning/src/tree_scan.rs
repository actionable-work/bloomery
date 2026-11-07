use crate::{AnalysisSession, ScanOutput, SourceTree, nix, rust, typescript};
use bloomery_model::{Diagnostic, config::ScannersConfig, sort_diagnostics};
use bloomery_workspace::source::TreeIndex;
use std::path::Path;

/// Discovery is syntactic: cfg, target and feature conditions are not evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanScope {
    Syntactic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryState {
    Disabled,
    NoMatchingFiles,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannerDiscovery {
    pub scanner: &'static str,
    pub state: DiscoveryState,
}

/// Static facts, never execution results or specification coverage claims.
#[derive(Debug, PartialEq, Eq)]
pub struct ScanAnalysis {
    pub scope: ScanScope,
    pub scanners: Vec<ScannerDiscovery>,
    pub output: ScanOutput,
    pub rust_conditions: Vec<crate::RustCondition>,
    pub rust_modules: Option<Vec<crate::RustModule>>,
}

impl ScanAnalysis {
    /// True only when every supported scanner completed syntactic discovery.
    pub fn is_complete(&self) -> bool {
        self.scanners
            .iter()
            .all(|scanner| scanner.state == DiscoveryState::Complete)
    }
}

/// Discover references and test bindings from caller-authorized content.
/// Locations are repository-relative. No filesystem or subprocess is required.
pub fn scan_tree(
    tree: &dyn SourceTree,
    config: &ScannersConfig,
) -> Result<ScanAnalysis, Vec<Diagnostic>> {
    scan_with_session(tree, config, &AnalysisSession::new(), None, false)
}

/// Collect local Rust predicate syntax using the same source ingestion pass.
pub fn scan_tree_with_conditions(
    tree: &dyn SourceTree,
    config: &ScannersConfig,
) -> Result<ScanAnalysis, Vec<Diagnostic>> {
    scan_with_session(tree, config, &AnalysisSession::new(), None, true)
}

/// Add direct root module context without expanding configured discovery globs.
pub fn scan_tree_with_rust_root(
    tree: &dyn SourceTree,
    config: &ScannersConfig,
    root: &str,
) -> Result<ScanAnalysis, Vec<Diagnostic>> {
    let session = AnalysisSession::new();
    let mut analysis = scan_with_session(tree, config, &session, None, true)?;
    if config.rust.enabled {
        analysis.rust_modules = Some(crate::modules::root_modules(tree, root, &session)?);
    }
    Ok(analysis)
}

pub(crate) fn scan_with_session(
    tree: &dyn SourceTree,
    config: &ScannersConfig,
    session: &AnalysisSession,
    filesystem_root: Option<&Path>,
    collect_conditions: bool,
) -> Result<ScanAnalysis, Vec<Diagnostic>> {
    config.validate().map_err(|error| vec![error])?;
    // Disabled scanners need neither an index nor file content.
    let index = if config.rust.enabled || config.playwright.enabled || config.nix.enabled {
        Some(
            TreeIndex::new(tree)
                .map_err(|error| vec![Diagnostic::new("SourceTreeError", error)])?,
        )
    } else {
        None
    };
    let mut output = ScanOutput::default();
    let mut rust_conditions = Vec::new();
    let mut diagnostics = Vec::new();
    let mut scanners = Vec::new();
    let mut nix_patterns = config.nix.paths.clone();
    nix_patterns.extend(config.nix.test_paths.clone());
    for (name, enabled, patterns) in [
        ("rust", config.rust.enabled, &config.rust.paths),
        (
            "playwright",
            config.playwright.enabled,
            &config.playwright.paths,
        ),
        ("nix", config.nix.enabled, &nix_patterns),
    ] {
        let selected = index
            .as_ref()
            .map(|index| index.matching_paths(patterns))
            .unwrap_or_default();
        let state = if !enabled {
            DiscoveryState::Disabled
        } else if selected.is_empty() {
            DiscoveryState::NoMatchingFiles
        } else {
            let found = match name {
                "rust" => rust::scan(
                    tree,
                    &selected,
                    session,
                    filesystem_root,
                    collect_conditions,
                )
                .map(|(output, conditions)| {
                    rust_conditions.extend(conditions);
                    output
                }),
                "playwright" => typescript::scan(tree, &selected, &config.playwright.tag_prefix),
                _ => nix::scan(
                    tree,
                    &selected,
                    &index
                        .as_ref()
                        .expect("enabled scanner index")
                        .matching_paths(&config.nix.test_paths),
                ),
            };
            match found {
                Ok(found) => output.absorb(found),
                Err(errors) => diagnostics.extend(errors),
            }
            DiscoveryState::Complete
        };
        scanners.push(ScannerDiscovery {
            scanner: name,
            state,
        });
    }
    if !diagnostics.is_empty() {
        sort_diagnostics(&mut diagnostics);
        return Err(diagnostics);
    }
    sort_output(&mut output);
    rust_conditions.sort_by(|a, b| a.test.cmp(&b.test));
    Ok(ScanAnalysis {
        scope: ScanScope::Syntactic,
        scanners,
        output,
        rust_conditions,
        rust_modules: None,
    })
}

fn sort_output(output: &mut ScanOutput) {
    output
        .evidence
        .sort_by(|a, b| (a.scanner, &a.location, &a.id).cmp(&(b.scanner, &b.location, &b.id)));
    for test in &mut output.tests {
        test.references.sort();
        test.references.dedup();
    }
    output
        .tests
        .sort_by(|a, b| (a.scanner, &a.location, &a.name).cmp(&(b.scanner, &b.location, &b.name)));
}
