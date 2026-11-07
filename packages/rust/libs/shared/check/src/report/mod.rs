//! Static repository analysis with deterministic, versioned report identities.
//!
//! ```
//! use bloomery_check::report::{analyze_tree, StageState};
//! use bloomery_model::Config;
//! use bloomery_workspace::source::MemoryTree;
//!
//! let tree = MemoryTree::new([("tests/example.rs", "#[test] fn example() {}")]).unwrap();
//! let report = analyze_tree(&tree, Config::default());
//! assert_eq!(report.version, 6);
//! assert_eq!(report.completeness.tree, StageState::Complete);
//! assert_eq!(report.scope.kind, "syntactic");
//! ```

mod cfg;
#[cfg(test)]
mod cfg_tests;
mod facts;
#[cfg(test)]
mod harness_tests;
pub use cfg::{ConditionRecord, ConditionState, RustCfg};
mod identity;
mod modules;
pub use modules::RustModuleRecord;
#[cfg(test)]
mod module_tests;
mod target;
pub use target::{CargoTestRecord, CargoTestSelection};
#[cfg(test)]
mod target_tests;
#[cfg(test)]
mod tests;
mod types;

use bloomery_model::{Config, Diagnostic};
use bloomery_scanning::{DiscoveryState, scan_tree_with_conditions, scan_tree_with_rust_root};
use bloomery_workspace::source::{FilesystemTree, SourceTree};
use identity::{Manifest, identity, snapshot};
use std::path::Path;
pub use types::*;

pub const REPORT_VERSION: u32 = 6;
pub const PARSER_CONTRACT: &str = "bloomery-static-syntax-v6";
pub const SPECIFICATION_SCHEMA: &str = "bloomery-grouped-specifications-v1";

/// Analyze a local repository with explicit configuration. Paths remain relative.
pub fn analyze(root: &Path, config: Config) -> StaticReport {
    analyze_tree(&FilesystemTree::new(root), config)
}

/// Analyze one caller-authorized tree without filesystem or subprocess access.
/// Configuration, content and analysis failures are explicit report findings.
pub fn analyze_tree(tree: &dyn SourceTree, config: Config) -> StaticReport {
    analyze_input(tree, config, None)
}

pub fn analyze_with_cfg(root: &Path, config: Config, cfg: RustCfg) -> StaticReport {
    analyze_tree_with_cfg(&FilesystemTree::new(root), config, cfg)
}

pub fn analyze_tree_with_cfg(tree: &dyn SourceTree, config: Config, cfg: RustCfg) -> StaticReport {
    analyze_input(tree, config, Some(cfg))
}

fn configure_scope(report: &mut StaticReport, cfg: Option<&RustCfg>) {
    if let Some(context) = cfg {
        report.scope.kind = "syntactic_with_rust_cfg".into();
        report.scope.target_resolution = "caller_supplied_facts".into();
        report.scope.feature_resolution = if context.features.is_some() {
            "caller_supplied_effective_features"
        } else {
            "unresolved"
        }
        .into();
        report.scope.conditional_compilation = "local_rust_predicates".into();
        report.scope.rust_cfg = Some(context.clone());
        if context.cargo_test.is_some() {
            report.scope.target_resolution = "declared_integration_test_direct_modules".into();
            report
                .scope
                .limitations
                .retain(|limit| limit != "required_features_targets" && limit != "module_graph");
            report.scope.limitations.extend([
                "cargo_target_auto_discovery".into(),
                "other_cargo_targets".into(),
                "transitive_module_membership".into(),
                "inline_external_modules".into(),
                "conditional_module_paths".into(),
            ]);
            report.scope.limitations.sort();
        }
    }
    report.inputs.scope = identity(&report.scope);
}

fn analyze_input(tree: &dyn SourceTree, config: Config, cfg: Option<RustCfg>) -> StaticReport {
    let mut report = empty_report(&config);
    configure_scope(&mut report, cfg.as_ref());
    if let Some(context) = &cfg
        && let Err(error) = context.validate()
    {
        report.completeness.rust_cfg = StageState::Failed;
        report
            .diagnostics
            .push(Diagnostic::new("RustCfgConfigurationError", error).into());
        return finish(report);
    }
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
    if let Some(cfg) = &cfg {
        target::project(&mut report, &tree, cfg, config.scanners.rust.enabled);
    }
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
    let root = report.cargo_test.as_ref().map(|target| target.path.clone());
    let analysis = if let Some(root) = &root {
        scan_tree_with_rust_root(&tree, &config.scanners, root)
    } else {
        scan_tree_with_conditions(&tree, &config.scanners)
    };
    match analysis {
        Ok(analysis) => {
            if let Some(declarations) = &analysis.rust_modules {
                modules::project(
                    &mut report,
                    &tree,
                    cfg.as_ref().expect("explicit target context"),
                    declarations,
                );
            }
            let complete = analysis.is_complete();
            let annotations = cfg
                .as_ref()
                .map(|context| cfg::annotations(context, &analysis.rust_conditions))
                .unwrap_or_default();
            let harness = cfg::harness_annotations(cfg.as_ref(), &analysis.rust_conditions);
            report.completeness.rust_harness =
                condition_completeness(analysis.scanners[0].state, &harness);
            if cfg.is_some() {
                report.completeness.rust_cfg =
                    condition_completeness(analysis.scanners[0].state, &annotations);
            }
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
            facts::add_discovery(
                &mut report,
                &analysis.output,
                context.as_ref(),
                complete && cfg.is_none(),
                &annotations,
                &harness,
            );
        }
        Err(errors) => {
            if root.is_some() {
                report.completeness.rust_modules = StageState::Failed;
            }
            report.completeness.discovery = StageState::Failed;
            report.completeness.rust_harness = if config.scanners.rust.enabled {
                StageState::Failed
            } else {
                StageState::Disabled
            };
            if cfg.is_some() {
                report.completeness.rust_cfg = StageState::Failed;
            }
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

fn condition_completeness(state: DiscoveryState, annotations: &cfg::Annotations) -> StageState {
    match state {
        DiscoveryState::Disabled => StageState::Disabled,
        DiscoveryState::NoMatchingFiles => StageState::Partial,
        DiscoveryState::Complete
            if annotations
                .values()
                .any(|condition| condition.state == ConditionState::Unresolved) =>
        {
            StageState::Partial
        }
        DiscoveryState::Complete => StageState::Complete,
    }
}

fn empty_report(config: &Config) -> StaticReport {
    StaticReport {
        version: REPORT_VERSION,
        identity: String::new(),
        inputs: InputIdentities {
            tree: None,
            configuration: identity(config),
            scope: String::new(),
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
            rust_cfg: None,
            limitations: vec![
                "cargo_feature_resolution".into(),
                "required_features_targets".into(),
                "module_graph".into(),
                "macro_expansion".into(),
            ],
        },
        cargo_test: None,
        completeness: Completeness {
            configuration: StageState::NotAttempted,
            tree: StageState::NotAttempted,
            specifications: StageState::NotAttempted,
            discovery: StageState::NotAttempted,
            relations: StageState::NotAttempted,
            rust_cfg: StageState::NotAttempted,
            rust_harness: StageState::NotAttempted,
            rust_target: StageState::NotAttempted,
            rust_modules: StageState::NotAttempted,
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
