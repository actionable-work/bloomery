use super::{
    Binding, BindingResolution, Coverage, Location, RequirementRecord, StageState, StaticReport,
    TestRecord, add_findings,
};
use bloomery_model::{Context, SourceLocation};
use bloomery_scanning::ScanOutput;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn requirements(context: &Context) -> Vec<RequirementRecord> {
    let mut records: Vec<_> = context
        .requirements()
        .map(|(area, feature, requirement)| RequirementRecord {
            id: requirement.entry.id.clone(),
            area: area.id.clone(),
            feature: feature.id.clone(),
            group: requirement.group.clone(),
            title: requirement.entry.title.clone(),
            manual: requirement.entry.manual,
            statement: requirement.entry.ears.to_statement(),
            location: Location::from(&SourceLocation::new(
                &requirement.path,
                Some(requirement.line),
            )),
            design_path: requirement.design.display.clone(),
            design_anchor: requirement.design.anchor.clone(),
            coverage: if requirement.entry.manual {
                Coverage::Manual
            } else {
                Coverage::Unresolved
            },
        })
        .collect();
    records.sort_by(|a, b| a.id.cmp(&b.id));
    records
}

pub(super) fn add_discovery(
    report: &mut StaticReport,
    output: &ScanOutput,
    context: Option<&Context>,
    complete: bool,
    annotations: &super::cfg::Annotations,
    harness: &super::cfg::Annotations,
) {
    let complete = complete
        && harness
            .values()
            .all(|condition| condition.state == super::ConditionState::Satisfied);
    let scoped = report.scope.rust_cfg.is_some();
    let selected = report
        .scope
        .rust_cfg
        .as_ref()
        .is_some_and(|cfg| cfg.cargo_test.is_some());
    let target = report.cargo_test.as_ref();
    let target_condition = |scanner: &str, path: &std::path::Path| {
        (selected && scanner == "rust").then(|| super::target::condition(target, path))
    };
    let declarations: BTreeMap<_, _> = report
        .requirements
        .iter()
        .map(|requirement| (requirement.id.clone(), requirement.manual))
        .collect();
    report.tests = output
        .tests
        .iter()
        .map(|site| TestRecord {
            scanner: site.scanner.into(),
            name: site.name.clone(),
            location: Location::from(&site.location),
            references: site.references.clone(),
            rust_cfg: annotations.get(&site.location).cloned(),
            rust_harness: harness.get(&site.location).cloned(),
            rust_target: target_condition(site.scanner, &site.location.path),
        })
        .collect();
    let enabled = context.is_some_and(|context| context.specs_enabled);
    report.bindings = output
        .evidence
        .iter()
        .map(|item| Binding {
            requirement_id: item.id.clone(),
            scanner: item.scanner.into(),
            location: Location::from(&item.location),
            rust_cfg: annotations.get(&item.location).cloned(),
            rust_harness: harness.get(&item.location).cloned(),
            rust_target: target_condition(item.scanner, &item.location.path),
            resolution: if !enabled {
                BindingResolution::SpecificationsUnavailable
            } else {
                match declarations.get(&item.id) {
                    Some(true) => BindingResolution::Manual,
                    Some(false) => BindingResolution::Declared,
                    None => BindingResolution::Unresolved,
                }
            },
        })
        .collect();
    if let Some(context) = context.filter(|context| context.specs_enabled) {
        let observed: BTreeSet<_> = report
            .bindings
            .iter()
            .filter(|binding| credits_coverage(binding, scoped))
            .map(|binding| binding.requirement_id.as_str())
            .collect();
        for requirement in &mut report.requirements {
            if !requirement.manual {
                requirement.coverage = if observed.contains(requirement.id.as_str()) {
                    Coverage::Bound
                } else if complete {
                    Coverage::Missing
                } else {
                    Coverage::Unresolved
                };
            }
        }
        let errors = crate::validate_evidence(context, &output.evidence)
            .err()
            .unwrap_or_default();
        add_findings(
            report,
            errors
                .into_iter()
                .filter(|error| complete || error.code != "MissingAutomatedTest")
                .collect(),
        );
        report.completeness.relations = if complete {
            StageState::Complete
        } else {
            StageState::Partial
        };
    }
}

fn credits_coverage(binding: &Binding, scoped: bool) -> bool {
    let satisfied =
        |condition: &super::ConditionRecord| condition.state == super::ConditionState::Satisfied;
    binding.scanner != "rust"
        || (binding.rust_harness.as_ref().is_some_and(satisfied)
            && (!scoped || binding.rust_cfg.as_ref().is_some_and(satisfied))
            && binding.rust_target.as_ref().is_none_or(satisfied))
}
