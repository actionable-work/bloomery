use crate::lockfile::{
    BloomeryContextPackage, BloomeryDependencyEdge, BloomeryLock, BloomeryPackage,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    resolve: Option<Resolve>,
    #[serde(default)]
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct CargoPackage {
    id: String,
    name: String,
    version: String,
    edition: Option<String>,
    #[serde(default)]
    license: Option<String>,
    #[serde(default)]
    license_file: Option<String>,
    #[serde(default)]
    targets: Vec<CargoTarget>,
    #[serde(default)]
    features: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    dependencies: Vec<CargoDependency>,
}

#[derive(Debug, Deserialize, Clone)]
struct CargoTarget {
    name: String,
    #[serde(default)]
    kind: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct CargoDependency {
    name: String,
    #[serde(default)]
    rename: Option<String>,
    #[serde(default)]
    optional: bool,
    #[serde(default = "default_true")]
    uses_default_features: bool,
    #[serde(default)]
    features: Vec<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    target: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Resolve {
    #[serde(default)]
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
    name: String,
    pkg: String,
    dep_kinds: Vec<DependencyKind>,
}

#[derive(Debug, Deserialize)]
struct DependencyKind {
    kind: Option<String>,
    target: Option<String>,
}

impl CargoPackage {
    fn library_name(&self) -> Option<&str> {
        self.targets
            .iter()
            .find(|target| {
                target.kind.iter().any(|kind| {
                    matches!(
                        kind.as_str(),
                        "lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "proc-macro"
                    )
                })
            })
            .map(|target| target.name.as_str())
    }
}

struct ActivatedDependency {
    id: String,
    edge: BloomeryDependencyEdge,
    features: BTreeSet<String>,
}

fn default_true() -> bool {
    true
}

fn package_key(package: &CargoPackage) -> String {
    format!("{}-{}", package.name, package.version)
}

fn sorted(values: BTreeSet<String>) -> Vec<String> {
    values.into_iter().collect()
}

struct Resolver {
    packages_by_id: BTreeMap<String, CargoPackage>,
    key_by_id: BTreeMap<String, String>,
    unified_features: BTreeMap<String, BTreeSet<String>>,
    resolved_deps: BTreeMap<String, Vec<ResolvedDependency>>,
    workspace_members: BTreeSet<String>,
}

impl Resolver {
    fn expand(&self, package: &CargoPackage, requested: &BTreeSet<String>) -> BTreeSet<String> {
        let mut active: BTreeSet<String> = requested
            .iter()
            .filter(|feature| package.features.contains_key(*feature))
            .cloned()
            .collect();
        loop {
            let mut added = false;
            for feature in active.clone() {
                let Some(entries) = package.features.get(&feature) else {
                    continue;
                };
                for entry in entries {
                    if entry.contains('/') || entry.starts_with("dep:") {
                        continue;
                    }
                    if package.features.contains_key(entry) && active.insert(entry.clone()) {
                        added = true;
                    }
                }
            }
            if !added {
                break;
            }
        }
        active
    }

    /// Features refer to declaration keys; extern bindings refer to Cargo's
    /// resolved names. Join them through the exact resolved package and unit.
    fn active_edges(
        &self,
        package: &CargoPackage,
        active: &BTreeSet<String>,
    ) -> Vec<ActivatedDependency> {
        let mut strong = BTreeSet::new();
        let mut requests: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut weak: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for feature in active {
            for entry in package.features.get(feature).into_iter().flatten() {
                if let Some(alias) = entry.strip_prefix("dep:") {
                    strong.insert(alias.to_owned());
                } else if let Some((left, feature)) = entry.split_once('/') {
                    let alias = left.trim_end_matches('?').to_owned();
                    if left.ends_with('?') {
                        weak.entry(alias).or_default().insert(feature.to_owned());
                    } else {
                        strong.insert(alias.clone());
                        requests
                            .entry(alias)
                            .or_default()
                            .insert(feature.to_owned());
                    }
                } else {
                    strong.insert(entry.clone());
                }
            }
        }

        let mut result = Vec::new();
        for declaration in &package.dependencies {
            let alias = declaration.rename.as_deref().unwrap_or(&declaration.name);
            let kind = declaration.kind.as_deref().unwrap_or("normal");
            if (declaration.optional && !strong.contains(alias))
                || (kind == "dev" && !self.workspace_members.contains(&package.id))
            {
                continue;
            }
            let mut features = requests.get(alias).cloned().unwrap_or_default();
            features.extend(weak.get(alias).into_iter().flatten().cloned());
            features.extend(declaration.features.iter().cloned());
            if declaration.uses_default_features {
                features.insert("default".to_owned());
            }
            for resolved in self.resolved_deps.get(&package.id).into_iter().flatten() {
                let dependency = &self.packages_by_id[&resolved.pkg];
                let extern_name = declaration
                    .rename
                    .as_ref()
                    .map(|name| name.replace('-', "_"));
                let expected_name = extern_name.as_deref().or_else(|| dependency.library_name());
                if dependency.name != declaration.name
                    || expected_name != Some(resolved.name.as_str())
                {
                    continue;
                }
                for unit in &resolved.dep_kinds {
                    if unit.kind.as_deref().unwrap_or("normal") == kind
                        && unit.target == declaration.target
                    {
                        result.push(ActivatedDependency {
                            id: resolved.pkg.clone(),
                            edge: BloomeryDependencyEdge {
                                package: self.key_by_id[&resolved.pkg].clone(),
                                extern_name: resolved.name.clone(),
                                kind: kind.to_owned(),
                                target: unit.target.clone(),
                            },
                            features: features.clone(),
                        });
                    }
                }
            }
        }
        result
    }

    fn dep_requests(
        &self,
        package: &CargoPackage,
        active: &BTreeSet<String>,
    ) -> BTreeMap<String, BTreeSet<String>> {
        let mut requests = BTreeMap::new();
        for dependency in self.active_edges(package, active) {
            requests
                .entry(dependency.id)
                .or_insert_with(BTreeSet::new)
                .extend(dependency.features);
        }
        requests
    }

    fn dependency_edges(
        &self,
        package: &CargoPackage,
        active: &BTreeSet<String>,
    ) -> Vec<BloomeryDependencyEdge> {
        self.active_edges(package, active)
            .into_iter()
            .map(|dependency| dependency.edge)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Resolve one fully-unified context per workspace member. Within a context
    /// every package has a single feature set, so crate instances never mix.
    fn resolve_contexts(&self) -> BTreeMap<String, BTreeMap<String, BloomeryContextPackage>> {
        let mut contexts = BTreeMap::new();
        for member in &self.workspace_members {
            let Some(package) = self.packages_by_id.get(member) else {
                continue;
            };
            let key = self
                .key_by_id
                .get(member)
                .cloned()
                .unwrap_or_else(|| package_key(package));
            let mut requested: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            let mut default = BTreeSet::new();
            if package.features.contains_key("default") {
                default.insert("default".to_owned());
            }
            requested.insert(member.clone(), default);

            loop {
                let mut changed = false;
                let ids: Vec<String> = requested.keys().cloned().collect();
                for id in ids {
                    let Some(package) = self.packages_by_id.get(&id) else {
                        continue;
                    };
                    let active = self.expand(package, &requested[&id]);
                    for (target, target_request) in self.dep_requests(package, &active) {
                        let existed = requested.contains_key(&target);
                        let entry = requested.entry(target).or_default();
                        let before = entry.len();
                        entry.extend(target_request);
                        if !existed || entry.len() != before {
                            changed = true;
                        }
                    }
                }
                if !changed {
                    break;
                }
            }

            let mut context = BTreeMap::new();
            for (id, features) in &requested {
                let Some(package) = self.packages_by_id.get(id) else {
                    continue;
                };
                let active = self.expand(package, features);
                let dependencies = self
                    .dep_requests(package, &active)
                    .keys()
                    .filter_map(|target| self.key_by_id.get(target).cloned())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                context.insert(
                    self.key_by_id[id].clone(),
                    BloomeryContextPackage {
                        features: sorted(active.clone()),
                        dependencies,
                        dependency_edges: self.dependency_edges(package, &active),
                    },
                );
            }
            contexts.insert(key, context);
        }
        contexts
    }
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

    let mut key_by_id = BTreeMap::new();
    let mut id_by_key = BTreeMap::new();
    for (id, package) in &packages_by_id {
        let key = package_key(package);
        if let Some(previous_identity) = id_by_key.get(&key)
            && previous_identity != id
        {
            return Err(format!(
                "unsupported Cargo resolution: distinct package identities '{previous_identity}' and '{id}' both map to bloomery.lock key '{key}'. The version-3 lock format keys packages by name-version; resolve the collision before syncing."
            ));
        }
        id_by_key.insert(key.clone(), id.clone());
        key_by_id.insert(id.clone(), key);
    }

    let mut unified_features = BTreeMap::new();
    let mut resolved_deps = BTreeMap::new();
    let mut node_ids = BTreeSet::new();
    for node in resolve.nodes {
        if !node_ids.insert(node.id.clone()) {
            return Err(format!(
                "cargo metadata returned resolved node '{}' more than once",
                node.id
            ));
        }
        if !packages_by_id.contains_key(&node.id) {
            return Err(format!(
                "resolved package identity '{}' was absent from cargo metadata packages",
                node.id
            ));
        }
        unified_features.insert(node.id.clone(), node.features.into_iter().collect());
        let deps = node
            .deps
            .into_iter()
            .map(|dependency| {
                if !packages_by_id.contains_key(&dependency.pkg) {
                    return Err(format!(
                        "resolved dependency identity '{}' referenced by '{}' was absent from cargo metadata packages",
                        dependency.pkg, node.id
                    ));
                }
                Ok(dependency)
            })
            .collect::<Result<Vec<_>, String>>()?;
        resolved_deps.insert(node.id, deps);
    }

    let workspace_members = if metadata.workspace_members.is_empty() {
        packages_by_id.keys().cloned().collect()
    } else {
        metadata.workspace_members.into_iter().collect()
    };

    let resolver = Resolver {
        packages_by_id,
        key_by_id,
        unified_features,
        resolved_deps,
        workspace_members,
    };

    let contexts = resolver.resolve_contexts();

    let mut lock_packages = BTreeMap::new();
    for (id, package) in &resolver.packages_by_id {
        let key = resolver.key_by_id[id].clone();
        let active = resolver
            .unified_features
            .get(id)
            .cloned()
            .unwrap_or_default();
        let dependencies = resolver
            .dep_requests(package, &active)
            .keys()
            .filter_map(|target| resolver.key_by_id.get(target).cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
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
        lock_packages.insert(
            key,
            BloomeryPackage {
                features: sorted(active.clone()),
                dependencies,
                dependency_edges: resolver.dependency_edges(package, &active),
                lib_name: package.library_name().map(str::to_owned),
                proc_macro,
                edition,
                license: package.license.clone(),
                license_file: package.license_file.clone(),
            },
        );
    }

    Ok(BloomeryLock {
        version: 3,
        cargo_lock_hash: sha256_normalized_crlf(cargo_lock),
        packages: lock_packages,
        contexts,
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
    use super::sha256_normalized_crlf;

    fn create_lock(
        metadata: &[u8],
        cargo_lock: &[u8],
    ) -> Result<crate::lockfile::BloomeryLock, String> {
        super::create_lock(
            &crate::naming_tests::default_target_metadata(metadata),
            cargo_lock,
        )
    }

    fn metadata(nodes: &str) -> Vec<u8> {
        format!(
            r#"{{"packages":[
                {{"id":"crate-a 1.0.0 (registry+https://example.invalid)","name":"crate-a","version":"1.0.0","edition":"2024","license":"MIT OR Apache-2.0","license_file":null,"targets":[{{"kind":["lib"]}}],"features":{{"default":["serde"],"serde":[]}},"dependencies":[{{"name":"crate-b","optional":false,"uses_default_features":true,"features":["derive"]}}]}},
              {{"id":"crate-b 2.0.0 (registry+https://example.invalid)","name":"crate-b","version":"2.0.0","edition":null,"license":null,"license_file":"LICENSE.txt","targets":[{{"kind":["proc-macro"]}}],"features":{{"derive":[]}},"dependencies":[]}}
            ],"workspace_members":["crate-a 1.0.0 (registry+https://example.invalid)"],"resolve":{{"nodes":[{nodes}]}}}}"#
        )
        .into_bytes()
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-017"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-018"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-019"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-020"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-047"))]
    fn resolved_metadata_builds_canonical_lock_records_and_normalizes_crlf() {
        let metadata = metadata(
            r#"{"id":"crate-a 1.0.0 (registry+https://example.invalid)","features":["default","serde"],"deps":[{"pkg":"crate-b 2.0.0 (registry+https://example.invalid)"}]},
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
        assert_eq!(crate_a.features, ["default", "serde"]);
        assert_eq!(crate_a.dependencies, ["crate-b-2.0.0"]);
        assert_eq!(crate_a.edition, "2024");
        assert_eq!(crate_a.license.as_deref(), Some("MIT OR Apache-2.0"));
        assert_eq!(crate_a.license_file, None);
        assert!(!crate_a.proc_macro);
        let crate_b = &lock.packages["crate-b-2.0.0"];
        assert_eq!(crate_b.features, ["derive"]);
        assert!(crate_b.dependencies.is_empty());
        assert_eq!(crate_b.edition, "2021");
        assert_eq!(crate_b.license, None);
        assert_eq!(crate_b.license_file.as_deref(), Some("LICENSE.txt"));
        assert!(crate_b.proc_macro);

        let context = &lock.contexts["crate-a-1.0.0"];
        assert_eq!(context["crate-a-1.0.0"].features, ["default", "serde"]);
        assert_eq!(context["crate-a-1.0.0"].dependencies, ["crate-b-2.0.0"]);
        assert_eq!(context["crate-b-2.0.0"].features, ["derive"]);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-043"))]
    fn only_activated_optional_dependencies_are_recorded() {
        let metadata = br#"{
          "packages":[
            {"id":"app 0.1.0 (path+file:///w)","name":"app","version":"0.1.0","edition":"2021","targets":[{"kind":["lib"]}],"features":{"default":["useit"],"useit":["dep:chosen"]},"dependencies":[
              {"name":"chosen","rename":null,"optional":true,"uses_default_features":true,"features":[],"kind":null},
              {"name":"ignored","rename":null,"optional":true,"uses_default_features":true,"features":[],"kind":null}
            ]},
            {"id":"chosen 1.0.0 (registry+https://example.invalid)","name":"chosen","version":"1.0.0","edition":"2021","targets":[{"kind":["lib"]}],"features":{},"dependencies":[]},
            {"id":"ignored 1.0.0 (registry+https://example.invalid)","name":"ignored","version":"1.0.0","edition":"2021","targets":[{"kind":["lib"]}],"features":{},"dependencies":[]}
          ],
          "workspace_members":["app 0.1.0 (path+file:///w)"],
          "resolve":{"nodes":[
            {"id":"app 0.1.0 (path+file:///w)","features":["default","useit"],"deps":[{"pkg":"chosen 1.0.0 (registry+https://example.invalid)"},{"pkg":"ignored 1.0.0 (registry+https://example.invalid)"}]},
            {"id":"chosen 1.0.0 (registry+https://example.invalid)","features":[],"deps":[]},
            {"id":"ignored 1.0.0 (registry+https://example.invalid)","features":[],"deps":[]}
          ]}
        }"#;
        let lock = create_lock(metadata, b"lock").expect("lock");
        assert_eq!(lock.packages["app-0.1.0"].dependencies, ["chosen-1.0.0"]);
        let context = &lock.contexts["app-0.1.0"];
        assert_eq!(context["app-0.1.0"].dependencies, ["chosen-1.0.0"]);
        assert!(!context.contains_key("ignored-1.0.0"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-044"))]
    fn distinct_members_resolve_shared_packages_in_their_own_context() {
        let metadata = br#"{
          "packages":[
            {"id":"app 0.1.0 (path+file:///w)","name":"app","version":"0.1.0","edition":"2021","targets":[{"kind":["lib"]}],"features":{},"dependencies":[
              {"name":"shared","rename":null,"optional":false,"uses_default_features":false,"features":[],"kind":null}
            ]},
            {"id":"other 0.1.0 (path+file:///w)","name":"other","version":"0.1.0","edition":"2021","targets":[{"kind":["lib"]}],"features":{},"dependencies":[
              {"name":"shared","rename":null,"optional":false,"uses_default_features":false,"features":["beta"],"kind":null}
            ]},
            {"id":"shared 1.0.0 (registry+https://example.invalid)","name":"shared","version":"1.0.0","edition":"2021","targets":[{"kind":["lib"]}],"features":{"alpha":[],"beta":[]},"dependencies":[]}
          ],
          "workspace_members":["app 0.1.0 (path+file:///w)","other 0.1.0 (path+file:///w)"],
          "resolve":{"nodes":[
            {"id":"app 0.1.0 (path+file:///w)","features":[],"deps":[{"pkg":"shared 1.0.0 (registry+https://example.invalid)"}]},
            {"id":"other 0.1.0 (path+file:///w)","features":[],"deps":[{"pkg":"shared 1.0.0 (registry+https://example.invalid)"}]},
            {"id":"shared 1.0.0 (registry+https://example.invalid)","features":["beta"],"deps":[]}
          ]}
        }"#;
        let lock = create_lock(metadata, b"lock").expect("lock");
        // The public `packages` view is the cargo-like union.
        assert_eq!(lock.packages["shared-1.0.0"].features, ["beta"]);
        assert!(
            lock.contexts["app-0.1.0"]["shared-1.0.0"]
                .features
                .is_empty()
        );
        assert_eq!(
            lock.contexts["other-0.1.0"]["shared-1.0.0"].features,
            ["beta"]
        );
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
        assert!(error.contains("version-3 lock format"));
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
