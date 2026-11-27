use bloomery_check_catalog::catalog::{self, CheckPlan, NixBackend, PlanRequest, SystemCatalog};
use bloomery_check_support::context::CheckError;
use bloomery_check_support::model::{Notice, valid_system_name};
use bloomery_cli_types::{MISSING_FLAKE_CODE, MISSING_FLAKE_MESSAGE, flake_present};
use bloomery_glob::may_match_prefix;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub struct Catalog {
    pub ids: Vec<String>,
    pub systems: Vec<String>,
    pub notices: Vec<Notice>,
    /// Catalogs read for the selected systems, keyed by system.
    pub catalogs: BTreeMap<String, SystemCatalog>,
}

/// Derivation plans for selected Nix checks, keyed by check ID, with their
/// output and direct dependency metadata.
#[derive(Default)]
pub struct PlannedCatalog {
    pub plans: BTreeMap<String, CheckPlan>,
    pub outputs_by_derivation: BTreeMap<String, Vec<String>>,
    pub dependencies: BTreeMap<String, Vec<String>>,
}

fn static_check_ids(root: &Path) -> Result<Vec<String>, CheckError> {
    let loaded = bloomery_model::config::load_with_raw(root)
        .map_err(|diagnostic| CheckError::operational(diagnostic.message))?;
    let mut checks = Vec::new();
    if loaded.config.spec_management_enabled(root)
        && bloomery_model::catalog::effective_value(
            loaded.raw.as_ref(),
            &["checks", "specifications", "enable"],
        )
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        checks.extend([
            "static:structure".to_owned(),
            "static:traceability".to_owned(),
        ]);
    }
    let health = bloomery_model::health::HealthConfig::from_raw(loaded.raw.as_ref())
        .map_err(|diagnostic| CheckError::operational(diagnostic.message))?;
    if health.enabled {
        checks.push("static:code-health".to_owned());
    }
    let clippy = bloomery_model::clippy::ClippySuppressionConfig::from_raw(loaded.raw.as_ref())
        .map_err(|diagnostic| CheckError::operational(diagnostic.message))?;
    if clippy.enabled {
        checks.push("static:clippy-suppressions".to_owned());
    }
    let unused =
        bloomery_model::unused_dependencies::UnusedDependencyConfig::from_raw(loaded.raw.as_ref())
            .map_err(|diagnostic| CheckError::operational(diagnostic.message))?;
    if unused.enabled {
        checks.push("static:unused-dependencies".to_owned());
    }
    Ok(checks)
}

pub fn warn_missing_check_controls(root: &Path) {
    let Ok(loaded) = bloomery_model::config::load_with_raw(root) else {
        return;
    };
    let Some(raw) = loaded.raw.as_ref() else {
        return;
    };
    for (key, check) in missing_check_controls(raw, loaded.config.spec_management_enabled(root)) {
        tracing::warn!(
            check = %check,
            key = %key,
            "missing check control; run `bloomery config upgrade` to explicitly enable supported checks"
        );
    }
}

pub fn missing_check_controls(
    raw: &toml::Value,
    specifications_available: bool,
) -> Vec<(String, String)> {
    let nix_checks_explicitly_disabled =
        bloomery_model::catalog::raw_at(raw, &["checks", "enable"]).and_then(toml::Value::as_bool)
            == Some(false);
    let mut controls = vec![
        ("checks.enable", "nix:*"),
        ("checks.packages.enable", "nix:package"),
        ("checks.crates.enable", "nix:crate-check"),
        ("checks.lockFreshness.enable", "nix:lock-validation"),
        (
            "checks.workspaceDependencies.enable",
            "nix:workspace-dependencies",
        ),
        ("checks.noDefaultFeatures.enable", "nix:no-default-features"),
        ("checks.codeHealth.enable", "static:code-health"),
        (
            "checks.clippySuppressions.enable",
            "static:clippy-suppressions",
        ),
        (
            "checks.unusedDependencies.enable",
            "static:unused-dependencies",
        ),
        (
            "checks.unusedDependencies.compiler",
            "nix:unused-dependencies",
        ),
    ];
    if specifications_available {
        controls.push(("checks.specifications.enable", "static:structure"));
    }
    controls
        .into_iter()
        .filter(|(key, _)| {
            !(nix_checks_explicitly_disabled
                && *key != "checks.enable"
                && *key != "checks.codeHealth.enable"
                && *key != "checks.clippySuppressions.enable"
                && *key != "checks.unusedDependencies.enable"
                && *key != "checks.specifications.enable")
        })
        .filter(|(key, _)| {
            let path = key.split('.').collect::<Vec<_>>();
            bloomery_model::catalog::raw_at(raw, &path).is_none()
        })
        .map(|(key, check)| (key.to_owned(), check.to_owned()))
        .collect()
}

/// Which catalog sources a selection needs before any Nix evaluation.
enum SelectionScope {
    /// Only static checks can match; Nix is not consulted.
    StaticOnly(Vec<String>),
    /// Nix checks may match on these effective systems.
    WithNix {
        static_ids: Vec<String>,
        systems: Vec<String>,
    },
}

fn selection_scope(
    root: &Path,
    selectors: &[String],
    requested_systems: &[String],
    backend: &dyn NixBackend,
) -> Result<SelectionScope, CheckError> {
    if !flake_present(root) {
        return Err(CheckError {
            code: MISSING_FLAKE_CODE,
            message: MISSING_FLAKE_MESSAGE.to_owned(),
        });
    }
    let static_ids = static_check_ids(root)?;
    if requested_systems.is_empty()
        && !selectors.is_empty()
        && selectors
            .iter()
            .all(|selector| !may_match_prefix(selector, "nix:"))
    {
        return Ok(SelectionScope::StaticOnly(static_ids));
    }
    let systems = effective_systems(root, requested_systems, backend)?;
    Ok(SelectionScope::WithNix {
        static_ids,
        systems,
    })
}

/// Select catalog IDs matched by any selector; every selector must match.
///
/// An empty selector list selects the whole catalog.
pub fn select_ids(catalog: &[String], selectors: &[String]) -> Result<Vec<String>, String> {
    if selectors.is_empty() {
        return Ok(catalog.to_vec());
    }

    let mut selected = std::collections::BTreeSet::new();
    for selector in selectors {
        let matches = catalog
            .iter()
            .filter(|id| bloomery_glob::matches(selector, id))
            .collect::<Vec<_>>();
        if matches.is_empty() {
            return Err(format!(
                "check selector '{selector}' matched no available check"
            ));
        }
        selected.extend(matches.into_iter().cloned());
    }
    Ok(selected.into_iter().collect())
}

/// Plan the selected Nix checks with one plan invocation per system that has
/// a selected check.
pub fn plan_selected(
    root: &Path,
    catalog: &Catalog,
    selected: &BTreeSet<String>,
    backend: &dyn NixBackend,
) -> Result<PlannedCatalog, CheckError> {
    let mut planned = PlannedCatalog::default();
    for (system, system_catalog) in &catalog.catalogs {
        let prefix = format!("nix:{system}:");
        let attributes = selected
            .iter()
            .filter_map(|id| id.strip_prefix(&prefix))
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        if attributes.is_empty() {
            continue;
        }
        let request = PlanRequest {
            catalog: system_catalog,
            attributes: &attributes,
        };
        let system_plan = backend
            .plan_checks(root, system, &request)
            .map_err(CheckError::operational)?;
        planned
            .outputs_by_derivation
            .extend(system_plan.outputs_by_derivation);
        planned.dependencies.extend(system_plan.dependencies);
        for plan in system_plan.plans {
            planned
                .plans
                .insert(catalog::nix_check_id(system, &plan.attribute), plan);
        }
    }
    Ok(planned)
}

pub fn discover_for_selection(
    root: &Path,
    selectors: &[String],
    requested_systems: &[String],
    backend: &dyn NixBackend,
) -> Result<Catalog, CheckError> {
    let (static_ids, systems) = match selection_scope(root, selectors, requested_systems, backend)?
    {
        SelectionScope::StaticOnly(static_ids) => {
            return Ok(Catalog {
                ids: static_ids,
                systems: Vec::new(),
                notices: Vec::new(),
                catalogs: BTreeMap::new(),
            });
        }
        SelectionScope::WithNix {
            static_ids,
            systems,
        } => (static_ids, systems),
    };
    let discovered =
        catalog::discover_catalog(root, &systems, backend).map_err(CheckError::operational)?;
    let mut ids = discovered.ids;
    ids.extend(static_ids);
    ids.sort();
    ids.dedup();
    Ok(Catalog {
        ids,
        systems,
        notices: discovered.notices,
        catalogs: discovered.catalogs,
    })
}

pub fn effective_systems(
    root: &Path,
    requested_systems: &[String],
    backend: &dyn NixBackend,
) -> Result<Vec<String>, CheckError> {
    if requested_systems.is_empty() {
        return backend
            .host_system(root)
            .map(|system| vec![system])
            .map_err(CheckError::operational);
    }
    let systems = requested_systems
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if let Some(invalid) = systems.iter().find(|system| !valid_system_name(system)) {
        return Err(CheckError::usage(format!(
            "invalid Nix system name '{invalid}'"
        )));
    }
    Ok(systems)
}

#[cfg(test)]
mod tests {
    use super::select_ids;
    use bloomery_glob::{matches as glob_matches, may_match_prefix};

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-002"))]
    fn exact_selectors_match_one_complete_catalog_id() {
        assert!(glob_matches("static:structure", "static:structure"));
        assert!(!glob_matches("static:struct", "static:structure"));
        assert!(!glob_matches("Static:structure", "static:structure"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-003"))]
    fn globs_are_anchored_case_sensitive_and_support_only_star_and_question() {
        assert!(glob_matches(
            "nix:*:core:?est",
            "nix:x86_64-linux:core:test"
        ));
        assert!(glob_matches("*", "anything"));
        assert!(glob_matches("a?c", "a/c"));
        assert!(!glob_matches("nix:*", "prefix-nix:system:check"));
        assert!(!glob_matches("a[bc]", "ab"));
        assert!(may_match_prefix("nix:*", "nix:"));
        assert!(may_match_prefix("*", "nix:"));
        assert!(!may_match_prefix("static:*", "nix:"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-004"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-005"))]
    fn selectors_form_a_deduplicated_union_and_reject_unmatched_patterns() {
        let catalog = vec![
            "nix:x86_64-linux:core:doc".to_owned(),
            "nix:x86_64-linux:core:test".to_owned(),
            "static:structure".to_owned(),
        ];
        assert_eq!(
            select_ids(
                &catalog,
                &["nix:*:core:*".to_owned(), "nix:*:core:test".to_owned()]
            )
            .expect("matching selection"),
            vec![
                "nix:x86_64-linux:core:doc".to_owned(),
                "nix:x86_64-linux:core:test".to_owned(),
            ]
        );
        assert!(select_ids(&catalog, &["missing:*".to_owned()]).is_err());
    }
}
