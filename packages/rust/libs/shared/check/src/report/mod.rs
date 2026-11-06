//! Static repository analysis with deterministic, versioned report identities.
//!
//! ```
//! use bloomery_check::report::{analyze_tree, StageState};
//! use bloomery_model::Config;
//! use bloomery_workspace::source::MemoryTree;
//!
//! let tree = MemoryTree::new([("tests/example.rs", "#[test] fn example() {}")]).unwrap();
//! let report = analyze_tree(&tree, Config::default());
//! assert_eq!(report.version, 2);
//! assert_eq!(report.completeness.tree, StageState::Complete);
//! assert_eq!(report.scope.kind, "syntactic");
//! ```

mod facts;
mod identity;
#[cfg(test)]
mod tests;
mod types;

use bloomery_model::{Config, Diagnostic};
use bloomery_scanning::{DiscoveryState, scan_tree};
use bloomery_workspace::source::{FilesystemTree, SourceTree};
use identity::{Manifest, identity, snapshot};
use std::path::Path;
pub use types::*;

pub const REPORT_VERSION: u32 = 2;
pub const PARSER_CONTRACT: &str = "bloomery-static-syntax-v2";
pub const SPECIFICATION_SCHEMA: &str = "bloomery-grouped-specifications-v1";

/// Analyze a local repository with explicit configuration. Paths remain relative.
pub fn analyze(root: &Path, config: Config) -> StaticReport {
    analyze_tree(&FilesystemTree::new(root), config)
}

/// Analyze one caller-authorized tree without filesystem or subprocess access.
/// Configuration, content and analysis failures are explicit report findings.
pub fn analyze_tree(tree: &dyn SourceTree, config: Config) -> StaticReport {
    let mut report = empty_report(&config);
    if let Err(error) = config.validate() {
        report.completeness.configuration = StageState::Failed;
        report.diagnostics.push(error.into());
        return finish(report);
    }
    report.completeness.configuration = StageState::Complete;
    let (tree, manifest) = match snapshot(tree) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            report.completeness.tree = StageState::Failed;
            report.diagnostics.push(error.into());
            return finish(report);
        }
    };
    report.completeness.tree = StageState::Complete;
    report.inputs.tree = Some(identity(&manifest));
    let context = match bloomery_workspace::load_tree(&tree, config.clone()) {
        Ok(context) => {
            report.completeness.specifications = if context.specs_enabled {
                StageState::Complete
            } else {
                StageState::Disabled
            };
            let specs = if context.specs_enabled {
                manifest.subtree(&config.specs_root(Path::new("")).expect("validated root"))
            } else {
                Manifest::empty()
            };
            report.inputs.specifications = Some(identity(&specs));
            report.requirements = facts::requirements(&context);
            Some(context)
        }
        Err(errors) => {
            report.completeness.specifications = StageState::Failed;
            add_findings(&mut report, errors);
            None
        }
    };
    match scan_tree(&tree, &config.scanners) {
        Ok(analysis) => {
            let complete = analysis.is_complete();
            report.completeness.discovery = if complete {
                StageState::Complete
            } else {
                StageState::Partial
            };
            report.scanners = analysis
                .scanners
                .into_iter()
                .map(|scanner| ScannerStatus {
                    scanner: scanner.scanner.into(),
                    state: match scanner.state {
                        DiscoveryState::Disabled => ScannerState::Disabled,
                        DiscoveryState::NoMatchingFiles => ScannerState::NoMatchingFiles,
                        DiscoveryState::Complete => ScannerState::Complete,
                    },
                })
                .collect();
            facts::add_discovery(&mut report, &analysis.output, context.as_ref(), complete);
        }
        Err(errors) => {
            report.completeness.discovery = StageState::Failed;
            for scanner in &mut report.scanners {
                if scanner.state != ScannerState::Disabled {
                    scanner.state = ScannerState::Unavailable;
                }
            }
            add_findings(&mut report, errors);
        }
    }
    finish(report)
}

fn empty_report(config: &Config) -> StaticReport {
    StaticReport {
        version: REPORT_VERSION,
        identity: String::new(),
        inputs: InputIdentities {
            tree: None,
            configuration: identity(config),
            specifications: None,
            parser_contract: PARSER_CONTRACT.into(),
            specification_schema: SPECIFICATION_SCHEMA.into(),
        },
        configuration: serde_json::to_value(config).expect("configuration serializes"),
        scope: Scope {
            kind: "syntactic".into(),
            target_resolution: "unsupported".into(),
            feature_resolution: "unsupported".into(),
            conditional_compilation: "unsupported".into(),
        },
        completeness: Completeness {
            configuration: StageState::NotAttempted,
            tree: StageState::NotAttempted,
            specifications: StageState::NotAttempted,
            discovery: StageState::NotAttempted,
            relations: StageState::NotAttempted,
        },
        scanners: [
            ("rust", config.scanners.rust.enabled),
            ("playwright", config.scanners.playwright.enabled),
            ("nix", config.scanners.nix.enabled),
        ]
        .into_iter()
        .map(|(scanner, enabled)| ScannerStatus {
            scanner: scanner.into(),
            state: if enabled {
                ScannerState::NotAttempted
            } else {
                ScannerState::Disabled
            },
        })
        .collect(),
        requirements: Vec::new(),
        tests: Vec::new(),
        bindings: Vec::new(),
        diagnostics: Vec::new(),
    }
}

pub(super) fn add_findings(report: &mut StaticReport, errors: Vec<Diagnostic>) {
    report
        .diagnostics
        .extend(errors.into_iter().map(Finding::from));
}

fn finish(mut report: StaticReport) -> StaticReport {
    report.diagnostics.sort();
    let mut payload = serde_json::to_value(&report).expect("report serializes");
    payload
        .as_object_mut()
        .expect("report object")
        .remove("identity");
    report.identity = identity(&payload);
    report
}
