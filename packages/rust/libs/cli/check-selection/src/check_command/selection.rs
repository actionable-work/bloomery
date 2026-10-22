use bloomery_check_catalog::check_command::catalog::{self, CheckPlan, NixBackend};
use bloomery_check_support::check_command::context::CheckError;
use bloomery_check_support::check_command::model::{
    Notice, selector_may_match_prefix, valid_system_name,
};
use bloomery_cli_output::{MISSING_FLAKE_CODE, MISSING_FLAKE_MESSAGE};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub struct Catalog {
    pub ids: Vec<String>,
    pub systems: Vec<String>,
    pub notices: Vec<Notice>,
}

/// Full execution catalog evaluation: check IDs plus the per-system derivation
/// plan and dependency graph resolved in one Nix evaluation per system.
pub struct PlannedCatalog {
    pub ids: Vec<String>,
    pub systems: Vec<String>,
    pub notices: Vec<Notice>,
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
                && *key != "checks.specifications.enable")
        })
        .filter(|(key, _)| {
            let path = key.split('.').collect::<Vec<_>>();
            bloomery_model::catalog::raw_at(raw, &path).is_none()
        })
        .map(|(key, check)| (key.to_owned(), check.to_owned()))
        .collect()
}

pub fn plan_for_selection(
    root: &Path,
    selectors: &[String],
    requested_systems: &[String],
    backend: &dyn NixBackend,
) -> Result<PlannedCatalog, CheckError> {
    if !root.join("flake.nix").is_file() {
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
            .all(|selector| !selector_may_match_prefix(selector, "nix:"))
    {
        return Ok(PlannedCatalog {
            ids: static_ids,
            systems: Vec::new(),
            notices: Vec::new(),
            plans: BTreeMap::new(),
            outputs_by_derivation: BTreeMap::new(),
            dependencies: BTreeMap::new(),
        });
    }

    let systems = effective_systems(root, requested_systems, backend)?;
    let mut ids = static_ids;
    let mut notices = Vec::new();
    let mut plans = BTreeMap::new();
    let mut outputs_by_derivation = BTreeMap::new();
    let mut dependencies = BTreeMap::new();
    for system in &systems {
        let planned = backend
            .plan_checks(root, system)
            .map_err(CheckError::operational)?;
        if planned.legacy {
            notices.push(Notice {
                code: "LegacyRecursiveCheckExcluded".to_owned(),
                message: format!(
                    "excluded legacy recursive check output for {system}: bloomery:check"
                ),
            });
        }
        outputs_by_derivation.extend(planned.outputs_by_derivation);
        dependencies.extend(planned.dependencies);
        for plan in planned.plans {
            let id = format!("nix:{system}:{}", plan.attribute);
            plans.insert(id.clone(), plan);
            ids.push(id);
        }
    }
    ids.sort();
    ids.dedup();
    notices.sort_by(|left, right| left.message.cmp(&right.message));
    notices.dedup_by(|left, right| left.code == right.code && left.message == right.message);
    Ok(PlannedCatalog {
        ids,
        systems,
        notices,
        plans,
        outputs_by_derivation,
        dependencies,
    })
}

pub fn discover_for_selection(
    root: &Path,
    selectors: &[String],
    requested_systems: &[String],
    backend: &dyn NixBackend,
) -> Result<Catalog, CheckError> {
    if !root.join("flake.nix").is_file() {
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
            .all(|selector| !selector_may_match_prefix(selector, "nix:"))
    {
        return Ok(Catalog {
            ids: static_ids,
            systems: Vec::new(),
            notices: Vec::new(),
        });
    }

    let systems = effective_systems(root, requested_systems, backend)?;
    let (mut ids, notices) =
        catalog::discover_catalog(root, &systems, backend).map_err(CheckError::operational)?;
    ids.extend(static_ids);
    ids.sort();
    ids.dedup();
    Ok(Catalog {
        ids,
        systems,
        notices,
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
