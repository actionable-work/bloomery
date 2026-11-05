#![cfg(test)]

use crate::metadata::create_lock;
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture(std::path::PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "bloomery-naming-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("fixture directory");
        Self(root)
    }

    fn write(&self, name: &str, content: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("clean naming fixture");
    }
}

#[test]
#[cfg_attr(any(), bloomery("CLI-SYNC-NAMING-001"))]
#[cfg_attr(any(), bloomery("CLI-SYNC-NAMING-002"))]
#[cfg_attr(any(), bloomery("CLI-SYNC-NAMING-003"))]
#[cfg_attr(any(), bloomery("CLI-SYNC-NAMING-004"))]
#[cfg_attr(any(), bloomery("CLI-SYNC-NAMING-005"))]
#[cfg_attr(any(), bloomery("CLI-SYNC-NAMING-008"))]
#[cfg_attr(any(), bloomery("CLI-SYNC-NAMING-009"))]
#[cfg_attr(any(), bloomery("CLI-SYNC-NAMING-010"))]
fn real_cargo_preserves_target_names_and_resolved_edge_bindings() {
    let fixture = Fixture::new();
    fixture.write(
        "Cargo.toml",
        "[workspace]\nmembers = [\"dep\", \"app\", \"bin\"]\nresolver = \"2\"\n",
    );
    fixture.write("dep/Cargo.toml", "[package]\nname = \"different-name\"\nversion = \"1.0.0\"\nedition = \"2021\"\n[lib]\nname = \"actual_library\"\n");
    fixture.write("dep/src/lib.rs", "pub fn value() {}\n");
    fixture.write("app/Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[target.'cfg(unix)'.dependencies]\nrenamed = { package = \"different-name\", path = \"../dep\" }\n");
    fixture.write("app/src/lib.rs", "pub fn value() {}\n");
    fixture.write("bin/Cargo.toml", "[package]\nname = \"bin-only\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[build-dependencies]\ndifferent-name = { path = \"../dep\" }\n");
    fixture.write("bin/src/main.rs", "fn main() {}\n");
    fixture.write("bin/build.rs", "fn main() {}\n");
    let metadata = Command::new("cargo")
        .args(["metadata", "--offline", "--format-version", "1"])
        .current_dir(&fixture.0)
        .output()
        .expect("cargo metadata");
    assert!(
        metadata.status.success(),
        "{}",
        String::from_utf8_lossy(&metadata.stderr)
    );
    let cargo_lock = fs::read(fixture.0.join("Cargo.lock")).unwrap();
    let lock = create_lock(&metadata.stdout, &cargo_lock).expect("generated lock");
    let text = String::from_utf8(crate::lockfile::serialize_candidate(&lock).unwrap()).unwrap();
    let data: toml::Value = toml::from_str(&text).unwrap();
    assert_eq!(data["version"].as_integer(), Some(3));
    assert_eq!(
        data["packages"]["different-name-1.0.0"]["lib-name"].as_str(),
        Some("actual_library")
    );
    assert!(data["packages"]["bin-only-0.1.0"].get("lib-name").is_none());
    let edges = data["packages"]["app-0.1.0"]["dependency-edges"]
        .as_array()
        .unwrap();
    assert!(
        edges
            .iter()
            .any(|edge| edge["extern-name"].as_str() == Some("renamed")
                && edge["target"].as_str() == Some("cfg(unix)")
                && edge["kind"].as_str() == Some("normal"))
    );
    let build_edges = data["packages"]["bin-only-0.1.0"]["dependency-edges"]
        .as_array()
        .unwrap();
    assert!(build_edges.iter().any(
        |edge| edge["extern-name"].as_str() == Some("actual_library")
            && edge["kind"].as_str() == Some("build")
    ));
    assert_eq!(
        data["packages"]["app-0.1.0"]["dependency-edges"],
        data["contexts"]["app-0.1.0"]["app-0.1.0"]["dependency-edges"]
    );
    let mut shuffled: serde_json::Value = serde_json::from_slice(&metadata.stdout).unwrap();
    shuffled["packages"].as_array_mut().unwrap().reverse();
    shuffled["resolve"]["nodes"]
        .as_array_mut()
        .unwrap()
        .reverse();
    let second = create_lock(&serde_json::to_vec(&shuffled).unwrap(), &cargo_lock).unwrap();
    assert_eq!(
        crate::lockfile::serialize_candidate(&lock).unwrap(),
        crate::lockfile::serialize_candidate(&second).unwrap()
    );
}

/// Complete the default-name metadata used by older unit fixtures. Tests of
/// custom names and malformed naming metadata use the producer directly.
pub(crate) fn default_target_metadata(bytes: &[u8]) -> Vec<u8> {
    let mut data: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    let mut names = std::collections::BTreeMap::new();
    for package in data["packages"].as_array_mut().into_iter().flatten() {
        let name = package["name"].as_str().unwrap().replace('-', "_");
        names.insert(package["id"].as_str().unwrap().to_owned(), name.clone());
        for target in package["targets"].as_array_mut().into_iter().flatten() {
            target["name"] = name.clone().into();
        }
    }
    for node in data
        .get_mut("resolve")
        .and_then(|resolve| resolve.get_mut("nodes"))
        .and_then(serde_json::Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        for dep in node["deps"].as_array_mut().into_iter().flatten() {
            dep["name"] = names
                .get(dep["pkg"].as_str().unwrap())
                .cloned()
                .unwrap_or_default()
                .into();
            dep["dep_kinds"] = serde_json::json!([{"kind":null,"target":null}]);
        }
    }
    serde_json::to_vec(&data).unwrap()
}

fn named_resolution() -> serde_json::Value {
    serde_json::json!({
        "workspace_members": ["app", "observer"],
        "packages": [
            {"id":"app", "name":"app", "version":"0.1.0", "edition":"2021",
             "targets":[{"name":"app", "kind":["lib"]}],
             "features":{"default":["dep:old", "old/flag", "unused?/flag"]},
             "dependencies":[
                 {"name":"different-name", "rename":"old", "optional":true, "uses_default_features":false},
                 {"name":"different-name", "rename":"new", "uses_default_features":false},
                 {"name":"unused-package", "rename":"unused", "optional":true, "uses_default_features":false}
             ]},
            {"id":"observer", "name":"observer", "version":"0.1.0", "edition":"2021",
             "targets":[{"name":"observer", "kind":["lib"]}],
             "dependencies":[{"name":"different-name", "rename":"alternate", "uses_default_features":false, "features":["beta"]}]},
            {"id":"first", "name":"different-name", "version":"1.0.0", "edition":"2021",
             "targets":[{"name":"actual_library", "kind":["lib"]}], "features":{"flag":[],"beta":[]}},
            {"id":"second", "name":"different-name", "version":"2.0.0", "edition":"2021",
             "targets":[{"name":"actual_library", "kind":["lib"]}], "features":{"flag":[]}},
            {"id":"unused", "name":"unused-package", "version":"1.0.0", "edition":"2021",
             "targets":[{"name":"unused_library", "kind":["lib"]}], "features":{"flag":[]}}
        ],
        "resolve":{"nodes":[
            {"id":"app", "features":["default"], "deps":[
                {"name":"old", "pkg":"first", "dep_kinds":[{"kind":null,"target":null}]},
                {"name":"new", "pkg":"second", "dep_kinds":[{"kind":null,"target":null}]},
                {"name":"unused", "pkg":"unused", "dep_kinds":[{"kind":null,"target":null}]}
            ]},
            {"id":"observer", "features":[], "deps":[{"name":"alternate","pkg":"first","dep_kinds":[{"kind":null,"target":null}]}]},
            {"id":"first", "features":["flag","beta"], "deps":[]},
            {"id":"second", "features":[], "deps":[]},
            {"id":"unused", "features":[], "deps":[]}
        ]}
    })
}

#[test]
#[cfg_attr(any(), bloomery("CLI-SYNC-NAMING-006"))]
#[cfg_attr(any(), bloomery("CLI-SYNC-NAMING-007"))]
#[cfg_attr(any(), bloomery("CLI-SYNC-NAMING-008"))]
#[cfg_attr(any(), bloomery("CLI-SYNC-NAMING-009"))]
#[cfg_attr(any(), bloomery("CLI-SYNC-NAMING-012"))]
#[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-043"))]
fn exact_resolved_names_preserve_versions_features_and_member_contexts() {
    let metadata = named_resolution();
    let lock = create_lock(&serde_json::to_vec(&metadata).unwrap(), b"lock").unwrap();
    let app = &lock.contexts["app-0.1.0"];
    let edges = &app["app-0.1.0"].dependency_edges;
    assert!(
        edges
            .iter()
            .any(|edge| edge.extern_name == "old" && edge.package == "different-name-1.0.0")
    );
    assert!(
        edges
            .iter()
            .any(|edge| edge.extern_name == "new" && edge.package == "different-name-2.0.0")
    );
    assert!(!edges.iter().any(|edge| edge.extern_name == "unused"));
    assert_eq!(
        app["app-0.1.0"].dependencies,
        ["different-name-1.0.0", "different-name-2.0.0"]
    );
    assert_eq!(app["different-name-1.0.0"].features, ["flag"]);
    let observer = &lock.contexts["observer-0.1.0"];
    assert_eq!(observer["different-name-1.0.0"].features, ["beta"]);
    assert_eq!(
        observer["observer-0.1.0"].dependency_edges[0].extern_name,
        "alternate"
    );
    assert_eq!(
        lock.packages["different-name-1.0.0"].lib_name.as_deref(),
        Some("actual_library")
    );
    crate::lockfile::serialize_candidate(&lock).expect("valid versioned edge graph");
}

#[test]
#[cfg_attr(any(), bloomery("CLI-SYNC-NAMING-011"))]
fn invalid_names_references_projections_and_conflicting_aliases_are_not_published() {
    let valid = create_lock(&serde_json::to_vec(&named_resolution()).unwrap(), b"lock").unwrap();
    let mut invalid = valid.clone();
    invalid
        .packages
        .get_mut("different-name-1.0.0")
        .unwrap()
        .lib_name = Some("bad-name".to_owned());
    assert!(crate::lockfile::serialize_candidate(&invalid).is_err());
    let mut invalid = valid.clone();
    invalid
        .packages
        .get_mut("app-0.1.0")
        .unwrap()
        .dependency_edges[0]
        .extern_name = "bad-name".to_owned();
    assert!(crate::lockfile::serialize_candidate(&invalid).is_err());
    let mut invalid = valid.clone();
    invalid
        .packages
        .get_mut("app-0.1.0")
        .unwrap()
        .dependencies
        .clear();
    assert!(crate::lockfile::serialize_candidate(&invalid).is_err());
    let mut invalid = valid.clone();
    invalid
        .packages
        .get_mut("different-name-1.0.0")
        .unwrap()
        .lib_name = None;
    assert!(crate::lockfile::serialize_candidate(&invalid).is_err());
    let mut invalid = valid;
    let app = invalid.packages.get_mut("app-0.1.0").unwrap();
    for edge in &mut app.dependency_edges {
        edge.extern_name = "conflict".to_owned();
    }
    app.dependency_edges.sort();
    assert!(
        crate::lockfile::serialize_candidate(&invalid)
            .unwrap_err()
            .contains("conflicting extern")
    );
}
