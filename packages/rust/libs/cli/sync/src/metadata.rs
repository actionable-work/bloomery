use crate::lockfile::{BloomeryLock, BloomeryPackage};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    resolve: Option<Resolve>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    version: String,
    edition: Option<String>,
    #[serde(default)]
    targets: Vec<CargoTarget>,
}

#[derive(Debug, Deserialize)]
struct CargoTarget {
    #[serde(default)]
    kind: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Resolve {
    nodes: Vec<ResolveNode>,
}

#[derive(Debug, Deserialize)]
struct ResolveNode {
    id: String,
    #[serde(default)]
    features: Vec<String>,
    #[serde(default)]
    deps: Vec<ResolvedDependency>,
}

#[derive(Debug, Deserialize)]
struct ResolvedDependency {
    pkg: String,
}

pub(crate) fn create_lock(metadata_json: &[u8], cargo_lock: &[u8]) -> Result<BloomeryLock, String> {
    let metadata: CargoMetadata = serde_json::from_slice(metadata_json)
        .map_err(|error| format!("unable to parse cargo metadata JSON: {error}"))?;
    let resolve = metadata
        .resolve
        .ok_or_else(|| "cargo metadata did not include a resolved dependency graph".to_owned())?;

    let mut packages_by_id = BTreeMap::new();
    for package in metadata.packages {
        let id = package.id.clone();
        if packages_by_id.insert(id.clone(), package).is_some() {
            return Err(format!(
                "cargo metadata returned package identity '{id}' more than once"
            ));
        }
    }

    let mut lock_packages = BTreeMap::new();
    let mut identity_by_key = BTreeMap::<String, String>::new();
    let mut node_ids = BTreeSet::new();

    for node in resolve.nodes {
        if !node_ids.insert(node.id.clone()) {
            return Err(format!(
                "cargo metadata returned resolved node '{}' more than once",
                node.id
            ));
        }
        let package = packages_by_id.get(&node.id).ok_or_else(|| {
            format!(
                "resolved package identity '{}' was absent from cargo metadata packages",
                node.id
            )
        })?;
        let key = format!("{}-{}", package.name, package.version);
        if let Some(previous_identity) = identity_by_key.get(&key)
            && previous_identity != &node.id
        {
            return Err(format!(
                "unsupported Cargo resolution: distinct package identities '{previous_identity}' and '{}' both map to bloomery.lock key '{key}'. The version-1 lock format keys packages by name-version; resolve the collision before syncing.",
                node.id
            ));
        }
        identity_by_key.insert(key.clone(), node.id.clone());

        let mut features = node.features;
        features.sort();
        features.dedup();

        let mut dependencies = BTreeSet::new();
        for dependency in node.deps {
            let dependency_package = packages_by_id.get(&dependency.pkg).ok_or_else(|| {
                format!(
                    "resolved dependency identity '{}' referenced by '{}' was absent from cargo metadata packages",
                    dependency.pkg, node.id
                )
            })?;
            dependencies.insert(format!(
                "{}-{}",
                dependency_package.name, dependency_package.version
            ));
        }

        let proc_macro = package
            .targets
            .iter()
            .any(|target| target.kind.iter().any(|kind| kind == "proc-macro"));
        let edition = package
            .edition
            .as_deref()
            .filter(|edition| !edition.is_empty())
            .unwrap_or("2021")
            .to_owned();

        let previous = lock_packages.insert(
            key.clone(),
            BloomeryPackage {
                features,
                dependencies: dependencies.into_iter().collect(),
                proc_macro,
                edition,
            },
        );
        if previous.is_some() {
            return Err(format!(
                "unsupported Cargo resolution: multiple resolved nodes map to bloomery.lock key '{key}'"
            ));
        }
    }

    Ok(BloomeryLock {
        version: 1,
        cargo_lock_hash: sha256_normalized_crlf(cargo_lock),
        packages: lock_packages,
    })
}

pub(crate) fn sha256_normalized_crlf(contents: &[u8]) -> String {
    let mut normalized = Vec::with_capacity(contents.len());
    let mut offset = 0;
    while offset < contents.len() {
        if contents[offset] == b'\r' && contents.get(offset + 1) == Some(&b'\n') {
            normalized.push(b'\n');
            offset += 2;
        } else {
            normalized.push(contents[offset]);
            offset += 1;
        }
    }

    let digest = Sha256::digest(normalized);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut hex, "{byte:02x}").expect("writing to a String cannot fail");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::{create_lock, sha256_normalized_crlf};

    fn metadata(nodes: &str) -> Vec<u8> {
        format!(
            r#"{{"packages":[
              {{"id":"crate-a 1.0.0 (registry+https://example.invalid)","name":"crate-a","version":"1.0.0","edition":"2024","targets":[{{"kind":["lib"]}}]}},
              {{"id":"crate-b 2.0.0 (registry+https://example.invalid)","name":"crate-b","version":"2.0.0","edition":null,"targets":[{{"kind":["proc-macro"]}}]}}
            ],"resolve":{{"nodes":[{nodes}]}}}}"#
        )
        .into_bytes()
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-017"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-018"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-019"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-020"))]
    fn resolved_metadata_builds_canonical_lock_records_and_normalizes_crlf() {
        let metadata = metadata(
            r#"{"id":"crate-a 1.0.0 (registry+https://example.invalid)","features":["zeta","alpha","alpha"],"deps":[{"pkg":"crate-b 2.0.0 (registry+https://example.invalid)"},{"pkg":"crate-b 2.0.0 (registry+https://example.invalid)"}]},
               {"id":"crate-b 2.0.0 (registry+https://example.invalid)","features":["derive"],"deps":[]}"#,
        );
        let lock = create_lock(&metadata, b"version = 4\r\n\r\n").expect("lock records");

        assert_eq!(
            lock.cargo_lock_hash,
            sha256_normalized_crlf(b"version = 4\n\n")
        );
        assert_eq!(
            lock.packages.keys().cloned().collect::<Vec<_>>(),
            ["crate-a-1.0.0", "crate-b-2.0.0"]
        );
        let crate_a = &lock.packages["crate-a-1.0.0"];
        assert_eq!(crate_a.features, ["alpha", "zeta"]);
        assert_eq!(crate_a.dependencies, ["crate-b-2.0.0"]);
        assert_eq!(crate_a.edition, "2024");
        assert!(!crate_a.proc_macro);
        let crate_b = &lock.packages["crate-b-2.0.0"];
        assert_eq!(crate_b.features, ["derive"]);
        assert!(crate_b.dependencies.is_empty());
        assert_eq!(crate_b.edition, "2021");
        assert!(crate_b.proc_macro);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-022"))]
    fn distinct_package_identities_with_a_colliding_key_are_rejected() {
        let mut json: serde_json::Value = serde_json::from_slice(&metadata(
            r#"{"id":"crate-a 1.0.0 (registry+https://example.invalid)","features":[],"deps":[]},
               {"id":"crate-b 2.0.0 (registry+https://example.invalid)","features":[],"deps":[]}"#,
        ))
        .expect("metadata JSON");
        let packages = json["packages"].as_array_mut().expect("packages");
        let mut colliding = packages[0].clone();
        colliding["id"] = serde_json::Value::String(
            "crate-a 1.0.0 (git+https://example.invalid#revision)".to_owned(),
        );
        packages.push(colliding);
        json["resolve"]["nodes"]
            .as_array_mut()
            .expect("nodes")
            .push(serde_json::json!({
                "id": "crate-a 1.0.0 (git+https://example.invalid#revision)",
                "features": [],
                "deps": []
            }));

        let error = create_lock(&serde_json::to_vec(&json).expect("serialize"), b"lock")
            .expect_err("colliding package identities");
        assert!(error.contains("distinct package identities"));
        assert!(error.contains("crate-a-1.0.0"));
        assert!(error.contains("version-1 lock format"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-018"))]
    fn lock_hash_normalizes_crlf_without_removing_other_carriage_returns() {
        assert_eq!(
            sha256_normalized_crlf(b"a\r\nb\rc\n"),
            sha256_normalized_crlf(b"a\nb\rc\n")
        );
        assert_ne!(
            sha256_normalized_crlf(b"a\r\nb\rc\n"),
            sha256_normalized_crlf(b"a\nbc\n")
        );
    }

    #[test]
    fn empty_resolved_graph_and_missing_metadata_graph_are_handled_explicitly() {
        let empty = br#"{"packages":[],"resolve":{"nodes":[]}}"#;
        assert!(
            create_lock(empty, b"lock")
                .expect("empty graph")
                .packages
                .is_empty()
        );
        assert!(
            create_lock(br#"{"packages":[],"resolve":null}"#, b"lock")
                .expect_err("missing resolve graph")
                .contains("resolved dependency graph")
        );
    }
}
