//! Workspace-wide clippy suppression discovery over Cargo targets.

use bloomery_model::Diagnostic;
use bloomery_scanning::{AnalysisSession, find_clippy_suppressions};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Scan every workspace member target for clippy suppressions not in `allow`.
///
/// Targets come from offline `cargo metadata --no-deps`; discovery failures are
/// reported as `ClippySourceError` diagnostics.
pub fn check_workspace_clippy_suppressions(
    root: &Path,
    allow: &[String],
) -> Result<(), Vec<Diagnostic>> {
    let metadata = cargo_metadata(root).map_err(source_error)?;
    let source_paths = workspace_target_sources(&metadata).map_err(source_error)?;
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

fn cargo_metadata(root: &Path) -> Result<Value, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args([
            "metadata",
            "--offline",
            "--no-deps",
            "--format-version=1",
            "--manifest-path",
        ])
        .arg(root.join("Cargo.toml"))
        .current_dir(root)
        .output()
        .map_err(|error| format!("Unable to discover Cargo workspace targets: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Cargo target discovery failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("Cargo returned invalid target metadata: {error}"))
}

/// Source roots of every scanned target that belongs to a workspace member.
fn workspace_target_sources(metadata: &Value) -> Result<Vec<PathBuf>, String> {
    let workspace_members = metadata
        .get("workspace_members")
        .and_then(Value::as_array)
        .ok_or_else(|| "Cargo metadata omitted workspace members".to_owned())?
        .iter()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| "Cargo metadata omitted packages".to_owned())?;
    let member_targets = packages
        .iter()
        .filter(|package| {
            package
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| workspace_members.contains(id))
        })
        .filter_map(|package| package.get("targets").and_then(Value::as_array))
        .flatten();
    Ok(member_targets
        .filter(|target| is_scanned_target(target))
        .filter_map(|target| target.get("src_path").and_then(Value::as_str))
        .map(PathBuf::from)
        .collect())
}

fn is_scanned_target(target: &Value) -> bool {
    target
        .get("kind")
        .and_then(Value::as_array)
        .is_some_and(|kinds| {
            kinds
                .iter()
                .filter_map(Value::as_str)
                .any(|kind| SCANNED_TARGET_KINDS.contains(&kind))
        })
}

#[cfg(test)]
mod tests {
    use super::workspace_target_sources;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn target_sources_cover_only_scanned_kinds_of_workspace_members() {
        let metadata = json!({
            "workspace_members": ["member"],
            "packages": [
                {"id": "member", "targets": [
                    {"kind": ["lib"], "src_path": "/w/src/lib.rs"},
                    {"kind": ["custom-build"], "src_path": "/w/build.rs"},
                    {"kind": ["cdylib"], "src_path": "/w/src/ffi.rs"}
                ]},
                {"id": "dependency", "targets": [
                    {"kind": ["lib"], "src_path": "/d/src/lib.rs"}
                ]}
            ]
        });
        assert_eq!(
            workspace_target_sources(&metadata).expect("sources"),
            vec![PathBuf::from("/w/src/lib.rs"), PathBuf::from("/w/build.rs")]
        );
        assert_eq!(
            workspace_target_sources(&json!({"packages": []})).expect_err("members"),
            "Cargo metadata omitted workspace members"
        );
        assert_eq!(
            workspace_target_sources(&json!({"workspace_members": []})).expect_err("packages"),
            "Cargo metadata omitted packages"
        );
    }
}
