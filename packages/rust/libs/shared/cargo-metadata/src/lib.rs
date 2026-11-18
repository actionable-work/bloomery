//! Typed projection of offline `cargo metadata --no-deps` output for the
//! workspace members a static check inspects.

use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Workspace packages reported by Cargo.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Metadata {
    workspace_members: Vec<String>,
    packages: Vec<Package>,
}

/// One Cargo package and the targets and dependencies its manifest declares.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub manifest_path: PathBuf,
    pub targets: Vec<Target>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

/// One compiled Cargo target.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Target {
    pub name: String,
    pub kind: Vec<String>,
    pub src_path: PathBuf,
}

/// One dependency declaration from a package manifest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Dependency {
    /// Package name of the dependency.
    pub name: String,
    /// Manifest key when it differs from the package name.
    pub rename: Option<String>,
    pub kind: Option<String>,
    pub target: Option<String>,
}

/// Cargo dependency section that declared a dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DependencyKind {
    Normal,
    Development,
    Build,
}

impl Dependency {
    pub fn kind(&self) -> DependencyKind {
        match self.kind.as_deref() {
            Some("dev") => DependencyKind::Development,
            Some("build") => DependencyKind::Build,
            _ => DependencyKind::Normal,
        }
    }

    /// Key naming the dependency in its manifest section.
    pub fn manifest_key(&self) -> &str {
        self.rename.as_deref().unwrap_or(&self.name)
    }
}

impl Target {
    pub fn has_kind(&self, kinds: &[&str]) -> bool {
        self.kind.iter().any(|kind| kinds.contains(&kind.as_str()))
    }
}

/// Target kinds that produce a Rust library other crates can link.
pub const LIBRARY_KINDS: [&str; 6] = ["lib", "rlib", "dylib", "cdylib", "staticlib", "proc-macro"];

impl Metadata {
    /// Run offline `cargo metadata --no-deps` for the workspace at `root`.
    ///
    /// Uses `$CARGO` when set so nested invocations reuse the active toolchain.
    pub fn load(root: &Path) -> Result<Self, String> {
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
        Self::parse(&output.stdout)
    }

    /// Parse Cargo metadata JSON.
    pub fn parse(json: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(json)
            .map_err(|error| format!("Cargo returned invalid target metadata: {error}"))
    }

    /// Packages that are members of the workspace, in Cargo's order.
    pub fn workspace_packages(&self) -> impl Iterator<Item = &Package> {
        let members = self
            .workspace_members
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        self.packages
            .iter()
            .filter(move |package| members.contains(package.id.as_str()))
    }

    /// Rust crate name of the library target of the named package, when the
    /// package is known to this metadata.
    pub fn library_crate_name(&self, package: &str) -> Option<&str> {
        self.packages
            .iter()
            .find(|candidate| candidate.name == package)?
            .targets
            .iter()
            .find(|target| target.has_kind(&LIBRARY_KINDS))
            .map(|target| target.name.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::{DependencyKind, Metadata};
    use std::path::Path;

    const METADATA: &str = r#"{
        "workspace_members": ["member"],
        "packages": [
            {"id": "member", "name": "member", "manifest_path": "/w/Cargo.toml",
             "targets": [{"name": "member", "kind": ["lib"], "src_path": "/w/src/lib.rs"}],
             "dependencies": [
                {"name": "dependency", "rename": null, "kind": null, "target": null},
                {"name": "serde_json", "rename": "json", "kind": "dev", "target": null},
                {"name": "cc", "rename": null, "kind": "build", "target": "cfg(unix)"}
             ]},
            {"id": "dependency", "name": "dependency", "manifest_path": "/d/Cargo.toml",
             "targets": [{"name": "dep_lib", "kind": ["proc-macro"], "src_path": "/d/src/lib.rs"}]}
        ]
    }"#;

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-002"))]
    fn workspace_packages_exclude_non_members() {
        let metadata = Metadata::parse(METADATA.as_bytes()).expect("metadata");
        let members = metadata
            .workspace_packages()
            .map(|package| package.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(members, ["member"]);
        let member = metadata.workspace_packages().next().expect("member");
        assert_eq!(member.manifest_path, Path::new("/w/Cargo.toml"));
        assert_eq!(member.targets[0].src_path, Path::new("/w/src/lib.rs"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-006"))]
    fn dependencies_expose_kind_manifest_key_and_library_name() {
        let metadata = Metadata::parse(METADATA.as_bytes()).expect("metadata");
        let member = metadata.workspace_packages().next().expect("member");
        let kinds = member
            .dependencies
            .iter()
            .map(|dependency| (dependency.manifest_key(), dependency.kind()))
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            [
                ("dependency", DependencyKind::Normal),
                ("json", DependencyKind::Development),
                ("cc", DependencyKind::Build),
            ]
        );
        assert_eq!(metadata.library_crate_name("dependency"), Some("dep_lib"));
        assert_eq!(metadata.library_crate_name("serde_json"), None);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-010"))]
    fn invalid_metadata_is_reported() {
        assert!(
            Metadata::parse(b"{}")
                .expect_err("missing fields")
                .starts_with("Cargo returned invalid target metadata")
        );
    }
}
