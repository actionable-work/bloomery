use super::{Assessment, CrateLinks, Site, policy_error};
use bloomery_model::specifications::SpecificationPolicy;
use bloomery_model::{Context, Diagnostic};
use bloomery_workspace::{cargo::ownership::selected_crates, source::SourceTree};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn assess(
    context: &Context,
    tree: &dyn SourceTree,
    policy: &SpecificationPolicy,
    assessment: &mut Assessment,
) {
    let crates = match selected_crates(tree, &context.config) {
        Ok(crates) => crates,
        Err(error) => {
            assessment.diagnostics.push(error);
            return;
        }
    };
    let selected: BTreeSet<_> = crates
        .iter()
        .map(|package| package.manifest.as_str())
        .collect();
    for manifest in policy.crates.keys() {
        if !selected.contains(manifest.as_str()) {
            assessment.diagnostics.push(policy_error(format!(
                "crate ownership names no selected workspace crate: {manifest}"
            )));
        }
    }
    let features: BTreeSet<_> = context
        .areas
        .iter()
        .flat_map(|area| {
            area.features
                .iter()
                .map(|feature| format!("{}/{}", area.id, feature.id))
        })
        .collect();
    for ownership in policy.crates.values() {
        for feature in &ownership.features {
            if !features.contains(feature) {
                assessment.diagnostics.push(policy_error(format!(
                    "crate ownership names an unknown feature: {feature}"
                )));
            }
        }
    }
    if !assessment
        .diagnostics
        .iter()
        .any(|finding| finding.code == "SpecificationPolicyError")
    {
        assessment.report.ownership_complete = true;
    }
    let owners: BTreeMap<_, _> = context
        .requirements()
        .filter(|(_, _, requirement)| !requirement.entry.manual)
        .map(|(area, feature, requirement)| {
            (
                requirement.entry.id.as_str(),
                format!("{}/{}", area.id, feature.id),
            )
        })
        .collect();
    for package in crates {
        let mapping = policy.crates.get(&package.manifest);
        let mut row = CrateLinks {
            manifest: package.manifest,
            name: package.name,
            features: mapping
                .map(|rule| rule.features.clone())
                .unwrap_or_default(),
            mapped: mapping.is_some(),
            excluded: mapping.is_some_and(|rule| rule.exclude),
            reason: mapping.and_then(|rule| rule.reason.clone()),
            requirements: 0,
            linked_tests: 0,
            min_requirements: mapping.map_or(0, |rule| rule.min_requirements.unwrap_or(1)),
            min_linked_tests: mapping.map_or(0, |rule| rule.min_linked_tests.unwrap_or(1)),
        };
        row.features.sort();
        if row.excluded {
            row.min_requirements = 0;
            row.min_linked_tests = 0;
        }
        let links: Vec<_> = assessment
            .report
            .requirements
            .iter()
            .filter(|requirement| {
                owners
                    .get(requirement.id.as_str())
                    .is_some_and(|feature| row.features.contains(feature))
            })
            .collect();
        row.requirements = links.len();
        row.linked_tests = links
            .iter()
            .flat_map(|requirement| &requirement.sites)
            .collect::<BTreeSet<&Site>>()
            .len();
        if assessment.report.ownership_complete && !row.excluded {
            enforce(policy, &row, assessment);
        }
        assessment.report.crates.push(row);
    }
}

fn enforce(policy: &SpecificationPolicy, row: &CrateLinks, assessment: &mut Assessment) {
    let finding = |code: &str, message| Diagnostic::new(code, message).at(&row.manifest, None);
    if !row.mapped && policy.require_crate_ownership {
        assessment.diagnostics.push(finding(
            "MissingCrateOwnership",
            format!(
                "Crate '{}' has no feature ownership mapping or exclusion",
                row.manifest
            ),
        ));
    }
    if row.requirements < row.min_requirements {
        assessment.diagnostics.push(finding(
            "InsufficientCrateRequirements",
            format!(
                "Crate '{}' owns {} automated requirements; minimum is {}",
                row.manifest, row.requirements, row.min_requirements
            ),
        ));
    }
    if assessment.report.discovery_complete && row.linked_tests < row.min_linked_tests {
        assessment.diagnostics.push(finding(
            "InsufficientCrateTests",
            format!(
                "Crate '{}' has {} feature-linked validating sites; minimum is {}",
                row.manifest, row.linked_tests, row.min_linked_tests
            ),
        ));
    }
}
