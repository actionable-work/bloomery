use super::{
    CargoTestRecord, ConditionRecord, ConditionState, Location, RustCfg, StageState, StaticReport,
    cfg,
};
use bloomery_model::Diagnostic;
use bloomery_scanning::RustModule;
use bloomery_workspace::source::{SourceTree, TreeIndex, resolve_relative_path};
use serde::Serialize;
use std::{collections::BTreeSet, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RustModuleRecord {
    pub name: String,
    pub location: Location,
    pub candidates: Vec<String>,
    pub path: Option<String>,
    pub condition: ConditionRecord,
}

pub(super) fn project(
    report: &mut StaticReport,
    tree: &dyn SourceTree,
    context: &RustCfg,
    declarations: &[RustModule],
) {
    let index = TreeIndex::new(tree).expect("validated analysis snapshot");
    let target = report.cargo_test.as_mut().expect("projected Cargo target");
    for declaration in declarations {
        let mut record = RustModuleRecord {
            name: declaration.name.clone(),
            location: Location::from(&declaration.location),
            candidates: Vec::new(),
            path: None,
            condition: cfg::combine(
                [
                    target.condition.clone(),
                    context.evaluate(&declaration.condition),
                ]
                .into_iter(),
                false,
            ),
        };
        let error = resolve(&mut record, declaration, &index, &target.path).err();
        if let Some(error) = error {
            record.condition.state = ConditionState::Unresolved;
            record.condition.unresolved.insert(error.clone());
            report.diagnostics.push(
                Diagnostic::new(
                    "RustModuleResolutionError",
                    format!("module {}: {error}", declaration.name),
                )
                .at_location(declaration.location.clone())
                .into(),
            );
        }
        target.modules.push(record);
    }
    report.completeness.rust_modules = if target
        .modules
        .iter()
        .any(|module| module.condition.state == ConditionState::Unresolved)
    {
        StageState::Partial
    } else {
        StageState::Complete
    };
}

fn resolve(
    record: &mut RustModuleRecord,
    declaration: &RustModule,
    index: &TreeIndex,
    root: &str,
) -> Result<(), String> {
    let candidates: Result<BTreeSet<_>, _> = declaration
        .candidates
        .iter()
        .map(|path| resolve_relative_path("", &path.to_string_lossy()))
        .collect();
    if record.condition.state == ConditionState::Rejected {
        record.candidates = candidates.unwrap_or_default().into_iter().collect();
        return Ok(());
    }
    if declaration.unsupported_path {
        return Err("unsupported_module_path".into());
    }
    record.candidates = candidates?.into_iter().collect();
    let existing: Vec<_> = record
        .candidates
        .iter()
        .filter(|path| index.is_file(Path::new(path)))
        .collect();
    match existing.as_slice() {
        [path] if path.as_str() == root => Err("self_referential_module_path".into()),
        [path] => {
            record.path = Some((*path).clone());
            Ok(())
        }
        [] => Err("missing_module_path".into()),
        _ => Err("ambiguous_module_path".into()),
    }
}

pub(super) fn membership(target: &CargoTestRecord, path: &Path) -> ConditionRecord {
    let routes: Vec<_> = target
        .modules
        .iter()
        .filter(|module| {
            module
                .path
                .as_deref()
                .is_some_and(|candidate| Path::new(candidate) == path)
                || (module.condition.state == ConditionState::Rejected
                    && module
                        .candidates
                        .iter()
                        .any(|candidate| Path::new(candidate) == path))
        })
        .map(|module| module.condition.clone())
        .collect();
    if routes.is_empty() {
        cfg::unknown("rust_module_membership")
    } else {
        cfg::combine(routes.into_iter(), true)
    }
}
