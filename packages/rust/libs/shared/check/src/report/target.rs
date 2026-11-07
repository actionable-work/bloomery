use super::{ConditionRecord, ConditionState, RustCfg, StageState, StaticReport, cfg};
use bloomery_scanning::CfgExpression;
use bloomery_workspace::{cargo, source::SourceTree};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoTestSelection {
    pub manifest: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CargoTestRecord {
    pub manifest: String,
    pub name: String,
    pub path: String,
    pub required_features: BTreeSet<String>,
    pub harness: bool,
    pub condition: ConditionRecord,
    pub modules: Vec<super::RustModuleRecord>,
}

pub(super) fn project(
    report: &mut StaticReport,
    tree: &dyn SourceTree,
    context: &RustCfg,
    enabled: bool,
) {
    let Some(selection) = &context.cargo_test else {
        return;
    };
    if !enabled {
        report.completeness.rust_target = StageState::Disabled;
        report.completeness.rust_modules = StageState::Disabled;
        return;
    }
    match cargo::load_test_target(tree, &selection.manifest, &selection.name) {
        Ok(target) => {
            let condition = if target.harness {
                context.evaluate(&CfgExpression::All(
                    target
                        .required_features
                        .iter()
                        .map(|feature| CfgExpression::Value("feature".into(), feature.clone()))
                        .collect(),
                ))
            } else {
                cfg::unknown("custom_test_harness")
            };
            report.completeness.rust_target = if condition.state == ConditionState::Unresolved {
                StageState::Partial
            } else {
                StageState::Complete
            };
            report.cargo_test = Some(CargoTestRecord {
                manifest: target.manifest,
                name: target.name,
                path: target.path,
                required_features: target.required_features,
                harness: target.harness,
                condition,
                modules: Vec::new(),
            });
        }
        Err(error) => {
            report.completeness.rust_target = StageState::Failed;
            report.diagnostics.push(error.into());
        }
    }
}

pub(super) fn condition(target: Option<&CargoTestRecord>, path: &Path) -> ConditionRecord {
    match target {
        Some(target) if Path::new(&target.path) == path => target.condition.clone(),
        Some(target) => super::modules::membership(target, path),
        None => cfg::unknown("cargo_test_target_unavailable"),
    }
}
