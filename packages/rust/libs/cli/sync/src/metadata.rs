use crate::lockfile::{BloomeryContextPackage, BloomeryLock, BloomeryPackage};
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
    targets: Vec<CargoTarget>,
    #[serde(default)]
    features: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    dependencies: Vec<CargoDependency>,
}

#[derive(Debug, Deserialize, Clone)]
struct CargoTarget {
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
    pkg: String,
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
    name_to_ids: BTreeMap<String, Vec<String>>,
    unified_features: BTreeMap<String, BTreeSet<String>>,
    candidates: BTreeMap<String, BTreeSet<String>>,
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

    fn dep_aliases<'a>(&self, package: &'a CargoPackage) -> BTreeMap<String, &'a CargoDependency> {
        let mut aliases: BTreeMap<String, &'a CargoDependency> = BTreeMap::new();
        for dependency in &package.dependencies {
            let alias = dependency
                .rename
                .clone()
                .unwrap_or_else(|| dependency.name.clone());
            let is_dev = dependency.kind.as_deref() == Some("dev");
            match aliases.get(&alias) {
                Some(existing) if existing.kind.as_deref() != Some("dev") && is_dev => {}
                _ => {
                    aliases.insert(alias, dependency);
                }
            }
        }
        aliases
    }

    fn find_target(
        &self,
        dependency: &CargoDependency,
        candidates: &BTreeSet<String>,
    ) -> Option<String> {
        if let Some(found) = candidates.iter().find(|id| {
            self.packages_by_id
                .get(*id)
                .is_some_and(|package| package.name == dependency.name)
        }) {
            return Some(found.clone());
        }
        let by_name = self.name_to_ids.get(&dependency.name)?;
        if by_name.len() == 1 {
            Some(by_name[0].clone())
        } else {
            None
        }
    }

    /// Compute the dependency edges activated by `active` features, keyed by the
    /// target package id. Only activated optional/weak/target-relevant edges are
    /// returned.
    fn dep_requests(
        &self,
        package: &CargoPackage,
        active: &BTreeSet<String>,
    ) -> BTreeMap<String, BTreeSet<String>> {
        let aliases = self.dep_aliases(package);
        let mut strong = BTreeSet::new();
        let mut requests: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut weak: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for feature in active {
            let Some(entries) = package.features.get(feature) else {
                continue;
            };
            for entry in entries {
                if let Some(dependency) = entry.strip_prefix("dep:") {
                    strong.insert(dependency.to_owned());
                } else if let Some((left, feature)) = entry.split_once('/') {
                    let is_weak = left.ends_with('?');
                    let alias = left.trim_end_matches('?');
                    if is_weak {
                        weak.entry(alias.to_owned())
                            .or_default()
                            .insert(feature.to_owned());
                    } else {
                        strong.insert(alias.to_owned());
                        requests
                            .entry(alias.to_owned())
                            .or_default()
                            .insert(feature.to_owned());
                    }
                } else if aliases.contains_key(entry) {
                    strong.insert(entry.clone());
                }
            }
        }
        for (alias, dependency) in &aliases {
            let activated = !dependency.optional || strong.contains(alias);
            if !activated {
                continue;
            }
            let entry = requests.entry(alias.clone()).or_default();
            entry.extend(dependency.features.iter().cloned());
            if dependency.uses_default_features {
                entry.insert("default".to_owned());
            }
        }
        for (alias, features) in &weak {
            let activated =
                aliases.get(alias).is_some_and(|dep| !dep.optional) || strong.contains(alias);
            if activated {
                requests
                    .entry(alias.clone())
                    .or_default()
                    .extend(features.iter().cloned());
            }
        }

        let empty = BTreeSet::new();
        let candidates = self.candidates.get(&package.id).unwrap_or(&empty);

        let mut edges = BTreeMap::new();
        for (alias, features) in requests {
            let Some(dependency) = aliases.get(&alias) else {
                continue;
            };
            if dependency.kind.as_deref() == Some("dev")
                && !self.workspace_members.contains(&package.id)
            {
                continue;
            }
            if let Some(target) = self.find_target(dependency, candidates) {
                edges
                    .entry(target)
                    .or_insert_with(BTreeSet::new)
                    .extend(features);
            }
        }
        edges
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
                        features: sorted(active),
                        dependencies,
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
    let mut name_to_ids: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (id, package) in &packages_by_id {
        let key = package_key(package);
        if let Some(previous_identity) = id_by_key.get(&key)
            && previous_identity != id
        {
            return Err(format!(
                "unsupported Cargo resolution: distinct package identities '{previous_identity}' and '{id}' both map to bloomery.lock key '{key}'. The version-1 lock format keys packages by name-version; resolve the collision before syncing."
            ));
        }
        id_by_key.insert(key.clone(), id.clone());
        key_by_id.insert(id.clone(), key);
        name_to_ids
            .entry(package.name.clone())
            .or_default()
            .push(id.clone());
    }
    for ids in name_to_ids.values_mut() {
        ids.sort();
    }

    let mut unified_features = BTreeMap::new();
    let mut candidates: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
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
                Ok(dependency.pkg)
            })
            .collect::<Result<BTreeSet<_>, String>>()?;
        candidates.insert(node.id, deps);
    }

    let workspace_members = if metadata.workspace_members.is_empty() {
        packages_by_id.keys().cloned().collect()
    } else {
        metadata.workspace_members.into_iter().collect()
    };

    let resolver = Resolver {
        packages_by_id,
        key_by_id,
        name_to_ids,
        unified_features,
        candidates,
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
                features: sorted(active),
                dependencies,
                proc_macro,
                edition,
            },
        );
    }

    Ok(BloomeryLock {
        version: 1,
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
    use super::{create_lock, sha256_normalized_crlf};

    fn metadata(nodes: &str) -> Vec<u8> {
        format!(
            r#"{{"packages":[
              {{"id":"crate-a 1.0.0 (registry+https://example.invalid)","name":"crate-a","version":"1.0.0","edition":"2024","targets":[{{"kind":["lib"]}}],"features":{{"default":["serde"],"serde":[]}},"dependencies":[{{"name":"crate-b","optional":false,"uses_default_features":true,"features":["derive"]}}]}},
              {{"id":"crate-b 2.0.0 (registry+https://example.invalid)","name":"crate-b","version":"2.0.0","edition":null,"targets":[{{"kind":["proc-macro"]}}],"features":{{"derive":[]}},"dependencies":[]}}
            ],"workspace_members":["crate-a 1.0.0 (registry+https://example.invalid)"],"resolve":{{"nodes":[{nodes}]}}}}"#
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
        assert!(!crate_a.proc_macro);
        let crate_b = &lock.packages["crate-b-2.0.0"];
        assert_eq!(crate_b.features, ["derive"]);
        assert!(crate_b.dependencies.is_empty());
        assert_eq!(crate_b.edition, "2021");
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
