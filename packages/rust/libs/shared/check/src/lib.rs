use bloomery_model::{Context, Diagnostic};
use bloomery_scanning::scan_all_with_session;
use std::collections::BTreeMap;

pub mod health;

pub use bloomery_scanning::AnalysisSession;
pub use bloomery_scanning::find_clippy_suppressions;
pub use bloomery_scanning::{HealthCrateMetrics, HealthSourceCrate, HealthTarget};

pub fn analyze_health(
    root: &std::path::Path,
    crates: &[HealthSourceCrate],
    duplicate_min_tokens: usize,
    session: &AnalysisSession,
) -> Result<Vec<HealthCrateMetrics>, Vec<Diagnostic>> {
    bloomery_scanning::analyze_code_health(root, crates, duplicate_min_tokens, session)
}

pub fn run(context: &Context) -> Result<(), Vec<Diagnostic>> {
    run_with_session(context, &AnalysisSession::new())
}

/// Run traceability with the Rust source ingestion cache shared by other
/// consumers in the same check command.
pub fn run_with_session(
    context: &Context,
    session: &AnalysisSession,
) -> Result<(), Vec<Diagnostic>> {
    let evidence = scan_all_with_session(context, session)?.evidence;
    let mut diagnostics = Vec::new();
    let mut declared = BTreeMap::new();
    for (area, feature, requirement) in context.requirements() {
        declared.insert(requirement.entry.id.clone(), (area, feature, requirement));
    }
    let mut references = BTreeMap::new();
    for item in &evidence {
        references
            .entry(item.id.clone())
            .or_insert_with(Vec::new)
            .push(item);
    }
    for (id, items) in &references {
        let Some((_, _, requirement)) = declared.get(id) else {
            for item in items {
                diagnostics.push(
                    bloomery_model::Diagnostic::new(
                        "OrphanTestReference",
                        format!(
                            "{} scanner references unknown requirement ID '{id}'",
                            item.scanner
                        ),
                    )
                    .at(item.location.path.clone(), item.location.line),
                );
            }
            continue;
        };
        if requirement.entry.manual {
            for item in items {
                diagnostics.push(
                    bloomery_model::Diagnostic::new(
                        "IllegalManualAutomation",
                        format!("Manual requirement '{id}' is referenced by automated evidence"),
                    )
                    .at(item.location.path.clone(), item.location.line)
                    .note(format!(
                        "Requirement is declared at {}:{}",
                        requirement.path.display(),
                        requirement.line
                    )),
                );
            }
        }
    }
    for (area, feature, requirement) in context.requirements() {
        if !requirement.entry.manual && !references.contains_key(&requirement.entry.id) {
            diagnostics.push(
                bloomery_model::Diagnostic::new(
                    "MissingAutomatedTest",
                    format!(
                        "Automated requirement '{}' has no linked tests",
                        requirement.entry.id
                    ),
                )
                .at(requirement.path.clone(), Some(requirement.line))
                .note(format!("Owner: {}/{}", area.id, feature.id)),
            );
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
