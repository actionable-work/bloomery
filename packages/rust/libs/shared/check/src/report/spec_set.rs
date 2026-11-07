use super::{Completeness, StageState, StaticReport, identity::identity};
use bloomery_model::{Config, Requirement};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

/// Derived evidence for the complete statically discovered specification set.
/// The enclosing report binds this manifest to the tree and parser contracts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpecSetManifest {
    pub version: u32,
    /// SHA-256 of the canonical payload with this field omitted.
    pub identity: String,
    pub source_root: String,
    pub source_identity: String,
    pub configuration: String,
    pub scope: String,
    pub active: BTreeMap<String, String>,
    pub bindings_identity: String,
    pub completeness: Completeness,
    pub diagnostics_identity: String,
}

pub(super) fn revision(requirement: &Requirement, feature: &Path) -> String {
    let entry = &requirement.entry;
    let design = requirement
        .design
        .path
        .strip_prefix(feature)
        .expect("loader resolves feature-local design references");
    identity(&serde_json::json!({
        "version": 1,
        "id": entry.id,
        "title": entry.title,
        "manual": entry.manual,
        "ears": entry.ears,
        "design": {
            "path": design.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/"),
            "anchor": requirement.design.anchor,
        },
    }))
}

pub(super) fn project(report: &StaticReport, config: &Config) -> Option<SpecSetManifest> {
    if !matches!(
        report.completeness.specifications,
        StageState::Complete | StageState::Disabled
    ) {
        return None;
    }
    let mut manifest = SpecSetManifest {
        version: 1,
        identity: String::new(),
        source_root: config
            .specs_root(Path::new(""))
            .expect("validated configuration")
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/"),
        source_identity: report.inputs.specifications.clone()?,
        configuration: report.inputs.configuration.clone(),
        scope: report.inputs.scope.clone(),
        active: report
            .requirements
            .iter()
            .map(|requirement| (requirement.id.clone(), requirement.revision.clone()))
            .collect(),
        bindings_identity: identity(&report.bindings),
        completeness: report.completeness.clone(),
        diagnostics_identity: identity(&report.diagnostics),
    };
    let mut payload = serde_json::to_value(&manifest).expect("manifest serializes");
    payload
        .as_object_mut()
        .expect("manifest object")
        .remove("identity");
    manifest.identity = identity(&payload);
    Some(manifest)
}
