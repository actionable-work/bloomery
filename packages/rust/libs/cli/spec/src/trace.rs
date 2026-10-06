use crate::{
    CandidateRecord, ReferenceRecord, SpecError, SpecOutcome, TraceRecord, display_path,
    find_requirement,
};
use bloomery_model::{Context, Evidence};
use std::collections::BTreeMap;
use std::path::Path;

pub(crate) fn trace(
    context: &Context,
    root: &Path,
    ids: &[String],
) -> Result<SpecOutcome, SpecError> {
    let output = bloomery_scanning::scan_all(context).map_err(SpecError::from_diagnostics)?;
    let mut by_id: BTreeMap<String, Vec<&Evidence>> = BTreeMap::new();
    for item in &output.evidence {
        by_id.entry(item.id.clone()).or_default().push(item);
    }
    let mut requirements = Vec::new();
    for id in ids {
        let (area, feature, requirement) = find_requirement(context, id)?;
        let mut references = by_id.get(id).cloned().unwrap_or_default();
        references.sort_by(|left, right| {
            (
                left.scanner,
                left.location.path.as_path(),
                left.location.line,
            )
                .cmp(&(
                    right.scanner,
                    right.location.path.as_path(),
                    right.location.line,
                ))
        });
        requirements.push(TraceRecord {
            id: requirement.entry.id.clone(),
            area: area.id.clone(),
            feature: feature.id.clone(),
            group: requirement.group.clone(),
            title: requirement.entry.title.clone(),
            references: references
                .into_iter()
                .map(|item| ReferenceRecord {
                    scanner: item.scanner.to_owned(),
                    path: display_path(root, &item.location.path),
                    line: item.location.line,
                })
                .collect(),
        });
    }
    Ok(SpecOutcome::Trace { requirements })
}

pub(crate) fn candidates(context: &Context, root: &Path) -> Result<SpecOutcome, SpecError> {
    let output = bloomery_scanning::scan_all(context).map_err(SpecError::from_diagnostics)?;
    let mut candidates = output
        .tests
        .iter()
        .filter(|site| site.references.is_empty())
        .map(|site| CandidateRecord {
            scanner: site.scanner.to_owned(),
            name: site.name.clone(),
            path: display_path(root, &site.location.path),
            line: site.location.line,
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        (left.scanner.as_str(), left.path.as_str(), left.line).cmp(&(
            right.scanner.as_str(),
            right.path.as_str(),
            right.line,
        ))
    });
    Ok(SpecOutcome::Candidates { candidates })
}
