//! Workspace-wide clippy suppression discovery over Cargo targets.

use bloomery_cargo_metadata::{Metadata, Package};
use bloomery_model::Diagnostic;
use bloomery_scanning::{AnalysisSession, find_clippy_suppressions};
use std::path::{Path, PathBuf};

/// Scan every workspace member target for clippy suppressions not in `allow`.
///
/// Targets come from offline `cargo metadata --no-deps`; discovery failures are
/// reported as `ClippySourceError` diagnostics.
pub fn check_workspace_clippy_suppressions(
    root: &Path,
    allow: &[String],
) -> Result<(), Vec<Diagnostic>> {
    let metadata = Metadata::load(root).map_err(source_error)?;
    let source_paths = workspace_target_sources(metadata.workspace_packages());
    find_clippy_suppressions(root, &source_paths, allow, &AnalysisSession::new())
}

/// Target kinds whose sources are scanned for suppressions.
const SCANNED_TARGET_KINDS: [&str; 7] = [
    "lib",
    "bin",
    "example",
    "test",
    "bench",
    "custom-build",
    "proc-macro",
];

fn source_error(message: String) -> Vec<Diagnostic> {
    vec![Diagnostic::new("ClippySourceError", message)]
}

/// Source roots of every scanned target of the given workspace members.
fn workspace_target_sources<'a>(packages: impl Iterator<Item = &'a Package>) -> Vec<PathBuf> {
    packages
        .flat_map(|package| &package.targets)
        .filter(|target| target.has_kind(&SCANNED_TARGET_KINDS))
        .map(|target| target.src_path.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::workspace_target_sources;
    use bloomery_cargo_metadata::Metadata;
    use std::path::PathBuf;

    #[test]
    fn target_sources_cover_only_scanned_kinds_of_workspace_members() {
        let metadata = Metadata::parse(
            br#"{
                "workspace_members": ["member"],
                "packages": [
                    {"id": "member", "name": "member", "manifest_path": "/w/Cargo.toml", "targets": [
                        {"name": "member", "kind": ["lib"], "src_path": "/w/src/lib.rs"},
                        {"name": "build-script-build", "kind": ["custom-build"], "src_path": "/w/build.rs"},
                        {"name": "ffi", "kind": ["cdylib"], "src_path": "/w/src/ffi.rs"}
                    ]},
                    {"id": "dependency", "name": "dependency", "manifest_path": "/d/Cargo.toml", "targets": [
                        {"name": "dependency", "kind": ["lib"], "src_path": "/d/src/lib.rs"}
                    ]}
                ]
            }"#,
        )
        .expect("metadata");
        assert_eq!(
            workspace_target_sources(metadata.workspace_packages()),
            vec![PathBuf::from("/w/src/lib.rs"), PathBuf::from("/w/build.rs")]
        );
    }
}
