use bloomery_model::{Context, Diagnostic};
use bloomery_scanning::scan_all;
use std::collections::BTreeMap;

pub fn run(context: &Context) -> Result<(), Vec<Diagnostic>> {
    let evidence = scan_all(context)?;
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
