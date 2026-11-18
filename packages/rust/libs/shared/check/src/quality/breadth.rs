use super::{Assessment, Site, TestLinks};
use bloomery_glob::path_matches;
use bloomery_model::specifications::{SpecificationPolicy, TestException};
use bloomery_model::{Context, Diagnostic, Requirement, SourceLocation};
use bloomery_scanning::ScanOutput;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[cfg(test)]
mod tests;

pub(super) fn assess(
    context: &Context,
    output: &ScanOutput,
    policy: &SpecificationPolicy,
    assessment: &mut Assessment,
) {
    let root = &context.root;
    let mut grouped: BTreeMap<Site, BTreeSet<String>> = BTreeMap::new();
    for requirement in &assessment.report.requirements {
        for site in &requirement.sites {
            grouped
                .entry(site.clone())
                .or_default()
                .insert(requirement.id.clone());
        }
    }
    let names: BTreeMap<_, _> = output
        .tests
        .iter()
        .map(|test| {
            (
                Site::new(test.scanner, &test.location, root),
                test.name.clone(),
            )
        })
        .collect();
    let declarations: BTreeMap<_, _> = context
        .requirements()
        .map(|(_, _, requirement)| (requirement.entry.id.as_str(), requirement))
        .collect();
    for (site, ids) in grouped {
        let name = names
            .get(&site)
            .cloned()
            .flatten()
            .or_else(|| match site.scanner.as_str() {
                "nix" => Path::new(&site.path)
                    .file_stem()?
                    .to_str()
                    .map(str::to_owned),
                _ => None,
            });
        let exception = matching_exception(policy, &site, name.as_deref());
        let row = TestLinks {
            site,
            name,
            requirement_ids: ids.into_iter().collect(),
            limit: exception.map_or(policy.max_requirements_per_test, |exception| {
                exception.max_requirements
            }),
            exception_reason: exception.map(|exception| exception.reason.clone()),
        };
        if row.requirement_ids.len() > row.limit {
            assessment
                .diagnostics
                .push(finding(root, &row, &declarations));
        }
        assessment.report.tests.push(row);
    }
}

fn finding(
    root: &Path,
    row: &TestLinks,
    declarations: &BTreeMap<&str, &Requirement>,
) -> Diagnostic {
    let mut location =
        SourceLocation::new(root.join(&row.site.path), Some(row.site.line.unwrap_or(1)));
    location.column = row.site.column;
    let mut finding = Diagnostic::new("TooManyRequirementsForTest", format!("Test '{}' references {} distinct automated requirements; limit is {}", row.name.as_deref().unwrap_or(&row.site.path), row.requirement_ids.len(), row.limit))
        .at_location(location)
        .note("Separate independently verifiable test stories while preserving meaningful assertions and requirement links.");
    for id in &row.requirement_ids {
        let requirement = declarations[id.as_str()];
        let path = requirement
            .path
            .strip_prefix(root)
            .unwrap_or(&requirement.path);
        finding = finding.note(format!(
            "{id} declared at {}:{}",
            path.display(),
            requirement.line
        ));
    }
    if let Some(reason) = &row.exception_reason {
        finding = finding.note(format!("Limit exception: {reason}"));
    }
    finding
}

fn matching_exception<'a>(
    policy: &'a SpecificationPolicy,
    site: &Site,
    name: Option<&str>,
) -> Option<&'a TestException> {
    [true, false].into_iter().find_map(|named| {
        policy.test_exceptions.values().find(|exception| {
            exception.name.is_some() == named
                && path_matches(&exception.path, Path::new(&site.path))
                && exception
                    .name
                    .as_deref()
                    .is_none_or(|expected| name == Some(expected))
        })
    })
}
