//! Authored traceability metrics. These associations do not establish execution coverage.

mod breadth;
mod ownership;
#[cfg(test)]
mod tests;

use bloomery_model::specifications::{SpecificationPolicy, UntiedMode};
use bloomery_model::{Context, Diagnostic, SourceLocation};
use bloomery_scanning::ScanOutput;
use bloomery_workspace::source::{SourceTree, glob_matches};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Site {
    pub scanner: String,
    pub path: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl Site {
    fn new(scanner: &str, location: &SourceLocation, root: &Path) -> Self {
        Self {
            scanner: scanner.into(),
            path: location
                .display_path(root)
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/"),
            line: (scanner != "nix").then_some(location.line).flatten(),
            column: (scanner != "nix").then_some(location.column).flatten(),
        }
    }

    fn note(&self) -> String {
        format!(
            "{} {}:{}:{}",
            self.scanner,
            self.path,
            self.line.unwrap_or(1),
            self.column.unwrap_or(1)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RequirementLinks {
    pub id: String,
    pub limit: usize,
    pub exception_reason: Option<String>,
    pub sites: Vec<Site>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TestLinks {
    pub site: Site,
    pub name: Option<String>,
    pub requirement_ids: Vec<String>,
    pub limit: usize,
    pub exception_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UntiedSite {
    pub site: Site,
    pub name: Option<String>,
    pub exception_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CrateLinks {
    pub manifest: String,
    pub name: String,
    pub features: Vec<String>,
    pub mapped: bool,
    pub excluded: bool,
    pub reason: Option<String>,
    pub requirements: usize,
    pub linked_tests: usize,
    pub min_requirements: usize,
    pub min_linked_tests: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QualityReport {
    pub discovery_complete: bool,
    pub ownership_complete: bool,
    pub untied_mode: UntiedMode,
    pub max_untied_tests: usize,
    pub requirements: Vec<RequirementLinks>,
    pub tests: Vec<TestLinks>,
    pub untied: Vec<UntiedSite>,
    pub crates: Vec<CrateLinks>,
}

pub struct Assessment {
    pub report: QualityReport,
    pub diagnostics: Vec<Diagnostic>,
}

/// Assess caller-supplied discovery without any source reads.
pub fn assess_links(
    context: &Context,
    output: &ScanOutput,
    complete: bool,
) -> Result<Assessment, Diagnostic> {
    let policy = bloomery_model::specifications::from_config(&context.config)?;
    let mut assessment = Assessment {
        report: QualityReport {
            discovery_complete: complete,
            ownership_complete: false,
            untied_mode: policy.untied_tests,
            max_untied_tests: policy.max_untied_tests,
            requirements: Vec::new(),
            tests: Vec::new(),
            untied: Vec::new(),
            crates: Vec::new(),
        },
        diagnostics: Vec::new(),
    };
    let sites = validating_sites(context, output);
    granularity(context, &policy, &sites, &mut assessment);
    breadth::assess(context, output, &policy, &mut assessment);
    untied(context, output, &policy, &mut assessment);
    Ok(assessment)
}

/// Add manifest-only ownership analysis using the same repository adapter.
pub fn assess(
    context: &Context,
    output: &ScanOutput,
    tree: &dyn SourceTree,
    complete: bool,
) -> Result<Assessment, Diagnostic> {
    let mut assessment = assess_links(context, output, complete)?;
    let policy = bloomery_model::specifications::from_config(&context.config)?;
    if policy.require_crate_ownership || !policy.crates.is_empty() {
        ownership::assess(context, tree, &policy, &mut assessment);
    }
    Ok(assessment)
}

fn validating_sites(context: &Context, output: &ScanOutput) -> BTreeMap<String, BTreeSet<Site>> {
    let mut sites: BTreeMap<String, BTreeSet<Site>> = BTreeMap::new();
    for test in &output.tests {
        if test.scanner != "nix" {
            for id in &test.references {
                sites.entry(id.clone()).or_default().insert(Site::new(
                    test.scanner,
                    &test.location,
                    &context.root,
                ));
            }
        }
    }
    for evidence in &output.evidence {
        if evidence.scanner == "nix" {
            sites
                .entry(evidence.id.clone())
                .or_default()
                .insert(Site::new("nix", &evidence.location, &context.root));
        }
    }
    sites
}

fn granularity(
    context: &Context,
    policy: &SpecificationPolicy,
    sites: &BTreeMap<String, BTreeSet<Site>>,
    assessment: &mut Assessment,
) {
    let declared: BTreeSet<_> = context
        .requirements()
        .filter(|(_, _, requirement)| !requirement.entry.manual)
        .map(|(_, _, requirement)| requirement.entry.id.as_str())
        .collect();
    for id in policy.requirement_exceptions.keys() {
        if !declared.contains(id.as_str()) {
            assessment.diagnostics.push(policy_error(format!(
                "requirement exception names no automated requirement: {id}"
            )));
        }
    }
    for (_, _, requirement) in context
        .requirements()
        .filter(|(_, _, requirement)| !requirement.entry.manual)
    {
        let id = &requirement.entry.id;
        let exception = policy.requirement_exceptions.get(id);
        let limit = exception.map_or(policy.max_tests_per_requirement, |exception| {
            exception.max_tests
        });
        let sites: Vec<_> = sites.get(id).into_iter().flatten().cloned().collect();
        if sites.len() > limit {
            let mut diagnostic = Diagnostic::new("TooManyTestsForRequirement", format!("Requirement '{id}' has {} distinct validating sites; limit is {limit}", sites.len()))
                .at(&requirement.path, Some(requirement.line))
                .note("Review the obligations and split independent stories while preserving meaningful tests.");
            for site in &sites {
                diagnostic = diagnostic.note(site.note());
            }
            if let Some(exception) = exception {
                diagnostic = diagnostic.note(format!("Limit exception: {}", exception.reason));
            }
            assessment.diagnostics.push(diagnostic);
        }
        assessment.report.requirements.push(RequirementLinks {
            id: id.clone(),
            limit,
            exception_reason: exception.map(|exception| exception.reason.clone()),
            sites,
        });
    }
    assessment
        .report
        .requirements
        .sort_by(|left, right| left.id.cmp(&right.id));
}

fn untied(
    context: &Context,
    output: &ScanOutput,
    policy: &SpecificationPolicy,
    assessment: &mut Assessment,
) {
    if policy.untied_tests == UntiedMode::Off {
        return;
    }
    let mut sites = BTreeMap::new();
    for test in output
        .tests
        .iter()
        .filter(|test| test.references.is_empty())
    {
        let site = Site::new(test.scanner, &test.location, &context.root);
        let reason = policy
            .untied_exceptions
            .iter()
            .find(|(pattern, exception)| {
                glob_matches(pattern, Path::new(&site.path))
                    && exception
                        .name
                        .as_ref()
                        .is_none_or(|name| test.name.as_ref() == Some(name))
            })
            .map(|(_, exception)| exception.reason.clone());
        sites.entry(site.clone()).or_insert(UntiedSite {
            site,
            name: test.name.clone(),
            exception_reason: reason,
        });
    }
    assessment.report.untied = sites.into_values().collect();
    let count = assessment
        .report
        .untied
        .iter()
        .filter(|site| site.exception_reason.is_none())
        .count();
    if policy.untied_tests == UntiedMode::Deny && count > policy.max_untied_tests {
        let mut diagnostic = Diagnostic::new(
            "UntiedTestsLimitExceeded",
            format!(
                "{count} nonexempt untied test sites; limit is {}",
                policy.max_untied_tests
            ),
        );
        for untied in assessment
            .report
            .untied
            .iter()
            .filter(|site| site.exception_reason.is_none())
        {
            diagnostic = diagnostic.note(untied.site.note());
        }
        assessment.diagnostics.push(diagnostic);
    }
}

fn policy_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("SpecificationPolicyError", message)
}
