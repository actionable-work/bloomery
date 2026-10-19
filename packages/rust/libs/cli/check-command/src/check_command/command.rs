use super::catalog::{CheckPlan, NixBackend, NixCli, discover_catalog};
use super::execution::run_checks;
use super::health::{run_retrieve as run_health, run_update as run_health_update};
use super::interrupt::InterruptFlag;
use super::model::{Notice, Outcome, RunStatus, selector_may_match_prefix, valid_system_name};
use super::output;
use super::retrieval::{run_details, run_failures, run_list};
use super::store;
use crate::output::TerminalFacts;
pub use bloomery_cli_types::{
    CheckArgs, CheckOperation, DetailsArgs, FailureArgs, HealthArgs, HealthUpdateArgs, ListArgs,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CheckError {
    pub(super) code: &'static str,
    pub(super) message: String,
}

pub(super) struct CheckContext<'a> {
    pub(super) root: &'a Path,
    pub(super) json_mode: bool,
    pub(super) color: crate::output::ColorMode,
    pub(super) no_write: bool,
    pub(super) backend: &'a dyn NixBackend,
    pub(super) interrupt: InterruptFlag,
    pub(super) cache_base: Option<&'a Path>,
    pub(super) terminal: TerminalFacts,
}

impl CheckError {
    pub(super) fn usage(message: impl Into<String>) -> Self {
        Self {
            code: "UsageError",
            message: message.into(),
        }
    }

    pub(super) fn operational(message: impl Into<String>) -> Self {
        Self {
            code: "OperationalError",
            message: message.into(),
        }
    }

    pub(super) fn retrieval(message: impl Into<String>) -> Self {
        Self {
            code: "RetrievalError",
            message: message.into(),
        }
    }
}

pub fn run(args: CheckArgs, json_mode: bool) -> ExitCode {
    let root = match std::env::current_dir() {
        Ok(root) => root,
        Err(error) => {
            let mut stdout = io::stdout().lock();
            let mut stderr = io::stderr().lock();
            return report_error(
                json_mode,
                &CheckError::operational(format!("unable to determine workspace root: {error}")),
                None,
                &mut stdout,
                &mut stderr,
            );
        }
    };
    let interrupt = InterruptFlag::install();
    let backend = NixCli::default();
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    run_at_with(
        args,
        CheckContext {
            root: &root,
            json_mode,
            color: crate::output::ColorMode::Auto,
            no_write: false,
            backend: &backend,
            interrupt,
            cache_base: None,
            terminal: TerminalFacts::from_environment(),
        },
        &mut stdout,
        &mut stderr,
    )
}

pub fn run_at(
    args: CheckArgs,
    json_mode: bool,
    root: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    run_at_with_color(
        args,
        json_mode,
        crate::output::ColorMode::Auto,
        root,
        stdout,
        stderr,
    )
}

/// Execute `bloomery check` with an explicit human-output color selection.
pub fn run_at_with_color(
    args: CheckArgs,
    json_mode: bool,
    color: crate::output::ColorMode,
    root: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    run_at_with_color_and_mode(args, json_mode, color, false, root, stdout, stderr)
}

pub fn run_at_with_color_and_mode(
    args: CheckArgs,
    json_mode: bool,
    color: crate::output::ColorMode,
    no_write: bool,
    root: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let backend = NixCli::default();
    run_at_with(
        args,
        CheckContext {
            root,
            json_mode,
            color,
            no_write,
            backend: &backend,
            interrupt: InterruptFlag::install(),
            cache_base: None,
            terminal: TerminalFacts::from_environment(),
        },
        stdout,
        stderr,
    )
}

pub fn run_at_with_cache(
    args: CheckArgs,
    json_mode: bool,
    root: &Path,
    cache_base: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let backend = NixCli::default();
    run_at_with(
        args,
        CheckContext {
            root,
            json_mode,
            color: crate::output::ColorMode::Auto,
            no_write: false,
            backend: &backend,
            interrupt: InterruptFlag::install(),
            cache_base: Some(cache_base),
            terminal: TerminalFacts::from_environment(),
        },
        stdout,
        stderr,
    )
}

/// Outcome of running the default check suite as a prerequisite gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateOutcome {
    pub status: RunStatus,
    pub failed_checks: Vec<String>,
    pub exit_code: u8,
}

/// Run the default `bloomery check` suite for `args.systems` and return a
/// structured outcome. The run is retained through the ordinary check store so
/// its failures remain retrievable with `check failures` and `check details`.
pub fn run_gate_at(
    args: CheckArgs,
    root: &Path,
    cache_base: Option<&Path>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> GateOutcome {
    run_gate_at_with_mode(args, root, cache_base, false, stdout, stderr)
}

pub fn run_gate_at_with_mode(
    args: CheckArgs,
    root: &Path,
    cache_base: Option<&Path>,
    no_write: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> GateOutcome {
    let backend = NixCli::default();
    run_gate_at_with_backend_mode(args, root, cache_base, &backend, no_write, stdout, stderr)
}

#[cfg(test)]
pub(super) fn run_gate_at_with_backend(
    args: CheckArgs,
    root: &Path,
    cache_base: Option<&Path>,
    backend: &dyn NixBackend,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> GateOutcome {
    run_gate_at_with_backend_mode(args, root, cache_base, backend, false, stdout, stderr)
}

pub(super) fn run_gate_at_with_backend_mode(
    args: CheckArgs,
    root: &Path,
    cache_base: Option<&Path>,
    backend: &dyn NixBackend,
    no_write: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> GateOutcome {
    run_at_with(
        args,
        CheckContext {
            root,
            json_mode: false,
            color: crate::output::ColorMode::Auto,
            no_write,
            backend,
            interrupt: InterruptFlag::install(),
            cache_base,
            terminal: TerminalFacts::from_environment(),
        },
        stdout,
        stderr,
    );
    let (status, failed_checks) =
        match store::RunStore::open(root, cache_base).and_then(|store| store.latest()) {
            Ok((record, _)) => (
                record.status,
                record
                    .outcomes
                    .iter()
                    .filter(|outcome| {
                        outcome.outcome == Outcome::Failed || outcome.outcome == Outcome::Blocked
                    })
                    .map(|outcome| outcome.id.clone())
                    .collect(),
            ),
            Err(_) => (RunStatus::Error, Vec::new()),
        };
    GateOutcome {
        exit_code: match status {
            RunStatus::Passed => 0,
            RunStatus::Failed => 1,
            RunStatus::Interrupted => 130,
            RunStatus::Error => 2,
        },
        status,
        failed_checks,
    }
}

pub(super) fn run_at_with(
    args: CheckArgs,
    context: CheckContext<'_>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    if context.no_write
        && matches!(
            args.operation.as_ref(),
            Some(CheckOperation::HealthUpdate(_))
        )
    {
        return report_error(
            context.json_mode,
            &CheckError::usage(
                "--no-write cannot be used with bloomery check health update because it writes workspace files",
            ),
            None,
            stdout,
            stderr,
        );
    }
    // Mirror the shared CLI preflight at the feature boundary so no direct
    // caller can execute or retrieve checks without a root flake.
    if !context.root.join("flake.nix").is_file() {
        return report_error(
            context.json_mode,
            &CheckError {
                code: crate::output::MISSING_FLAKE_CODE,
                message: crate::output::MISSING_FLAKE_MESSAGE.to_owned(),
            },
            None,
            stdout,
            stderr,
        );
    }
    if let Err(error) = validate_args(&args) {
        return report_error(context.json_mode, &error, None, stdout, stderr);
    }
    let CheckContext {
        root: requested_root,
        json_mode,
        color,
        no_write,
        backend,
        interrupt,
        cache_base,
        terminal,
    } = context;
    let root = match fs::canonicalize(requested_root) {
        Ok(root) => root,
        Err(error) => {
            return report_error(
                json_mode,
                &CheckError::operational(format!("unable to resolve workspace root: {error}")),
                None,
                stdout,
                stderr,
            );
        }
    };

    let context = CheckContext {
        root: &root,
        json_mode,
        color,
        no_write,
        backend,
        interrupt,
        cache_base,
        terminal,
    };
    let CheckArgs {
        operation,
        selectors,
        systems,
        jobs,
        fail_fast,
    } = args;
    match operation {
        Some(CheckOperation::List(list)) => run_list(
            context.root,
            list,
            json_mode,
            color,
            stdout,
            stderr,
            context.backend,
        ),
        Some(CheckOperation::Failures(failures)) => run_failures(
            context.root,
            failures,
            json_mode,
            color,
            stdout,
            stderr,
            context.cache_base,
        ),
        Some(CheckOperation::Details(details)) => run_details(
            context.root,
            details,
            json_mode,
            color,
            stdout,
            stderr,
            context.cache_base,
            context.backend,
        ),
        Some(CheckOperation::Health(health)) => run_health(&context, health, stdout, stderr),
        Some(CheckOperation::HealthUpdate(update)) => {
            run_health_update(&context, update, stdout, stderr)
        }
        None => run_checks(
            CheckArgs {
                operation: None,
                selectors,
                systems,
                jobs,
                fail_fast,
            },
            &context,
            stdout,
            stderr,
        ),
    }
}

pub(super) fn validate_args(args: &CheckArgs) -> Result<(), CheckError> {
    if args.operation.is_some()
        && (!args.selectors.is_empty()
            || !args.systems.is_empty()
            || args.jobs.is_some()
            || args.fail_fast)
    {
        return Err(CheckError::usage(
            "execution options --check, --system, --jobs, and --fail-fast are not accepted before retrieval subcommands",
        ));
    }
    if args.jobs == Some(0) {
        return Err(CheckError::usage("--jobs must be a positive integer"));
    }
    for selector in &args.selectors {
        if selector.is_empty() {
            return Err(CheckError::usage("--check selectors must not be empty"));
        }
    }
    for system in &args.systems {
        if !valid_system_name(system) {
            return Err(CheckError::usage(format!(
                "invalid Nix system name '{system}'"
            )));
        }
    }
    match &args.operation {
        Some(CheckOperation::List(list)) => {
            validate_page_values(list.offset, list.limit)?;
            if list.selectors.iter().any(String::is_empty) {
                return Err(CheckError::usage("--check selectors must not be empty"));
            }
            for system in &list.systems {
                if !valid_system_name(system) {
                    return Err(CheckError::usage(format!(
                        "invalid Nix system name '{system}'"
                    )));
                }
            }
        }
        Some(CheckOperation::Failures(failures)) => {
            validate_page_values(failures.offset, failures.limit)?;
        }
        Some(CheckOperation::Details(details)) => {
            validate_page_values(details.offset.unwrap_or(0), details.limit)?;
            if details.failure.is_empty() {
                return Err(CheckError::usage("failure ID must not be empty"));
            }
        }
        Some(CheckOperation::Health(health)) => {
            validate_page_values(health.offset, health.limit)?;
        }
        Some(CheckOperation::HealthUpdate(_)) => {}
        None => {}
    }
    Ok(())
}

fn validate_page_values(_offset: usize, limit: usize) -> Result<(), CheckError> {
    if limit == 0 {
        return Err(CheckError::usage("--limit must be a positive integer"));
    }
    Ok(())
}

pub(super) struct Catalog {
    pub(super) ids: Vec<String>,
    pub(super) systems: Vec<String>,
    pub(super) notices: Vec<Notice>,
}

/// Full execution catalog evaluation: check IDs plus the per-system derivation
/// plan and dependency graph resolved in one Nix evaluation per system.
pub(super) struct PlannedCatalog {
    pub(super) ids: Vec<String>,
    pub(super) systems: Vec<String>,
    pub(super) notices: Vec<Notice>,
    pub(super) plans: BTreeMap<String, CheckPlan>,
    pub(super) outputs_by_derivation: BTreeMap<String, Vec<String>>,
    pub(super) dependencies: BTreeMap<String, Vec<String>>,
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

pub(super) fn warn_missing_check_controls(root: &Path) {
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

pub(super) fn missing_check_controls(
    raw: &toml::Value,
    specifications_available: bool,
) -> Vec<(String, String)> {
    let nix_checks_explicitly_disabled =
        bloomery_model::catalog::raw_at(raw, &["checks", "enable"]).and_then(toml::Value::as_bool)
            == Some(false);
    let mut controls = vec![
        ("checks.enable", "nix:*"),
        ("checks.includePackageChecks", "nix:package"),
        ("checks.crateCheck", "nix:crate-check"),
        ("checks.throwOnOutOfDate", "nix:lock-validation"),
        ("checks.workspaceDependencies", "nix:workspace-dependencies"),
        ("checks.noDefaultFeatures", "nix:no-default-features"),
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

pub(super) fn plan_for_selection(
    root: &Path,
    selectors: &[String],
    requested_systems: &[String],
    backend: &dyn NixBackend,
) -> Result<PlannedCatalog, CheckError> {
    if !root.join("flake.nix").is_file() {
        return Err(CheckError {
            code: crate::output::MISSING_FLAKE_CODE,
            message: crate::output::MISSING_FLAKE_MESSAGE.to_owned(),
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

pub(super) fn discover_for_selection(
    root: &Path,
    selectors: &[String],
    requested_systems: &[String],
    backend: &dyn NixBackend,
) -> Result<Catalog, CheckError> {
    if !root.join("flake.nix").is_file() {
        return Err(CheckError {
            code: crate::output::MISSING_FLAKE_CODE,
            message: crate::output::MISSING_FLAKE_MESSAGE.to_owned(),
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
        discover_catalog(root, &systems, backend).map_err(CheckError::operational)?;
    ids.extend(static_ids);
    ids.sort();
    ids.dedup();
    Ok(Catalog {
        ids,
        systems,
        notices,
    })
}

pub(super) fn effective_systems(
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

pub(super) fn write_page(
    json_mode: bool,
    color: crate::output::ColorMode,
    bytes: Vec<u8>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let bytes = if json_mode {
        bytes
    } else {
        crate::output::colorize_check(
            &String::from_utf8_lossy(&bytes),
            crate::output::color_enabled(crate::output::Stream::Stdout, false, color),
        )
        .into_bytes()
    };
    match stdout.write_all(&bytes).and_then(|()| stdout.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(stderr, "bloomery: unable to write check output: {error}");
            ExitCode::from(2)
        }
    }
}

pub(super) fn report_error(
    json_mode: bool,
    error: &CheckError,
    run: Option<&str>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    if json_mode {
        let mut document = output::error_document(error.code, &error.message, run);
        if let Some(run) = run {
            document["error"]["details"]["run"] = serde_json::Value::String(run.to_owned());
        }
        match output::compact_json(&document) {
            Ok(bytes) => {
                let _ = stdout.write_all(&bytes).and_then(|()| stdout.flush());
            }
            Err(message) => {
                let _ = writeln!(stderr, "bloomery: {message}");
            }
        }
    } else {
        let message = format!("error: {}", error.message);
        let rendered = crate::output::colorize_error(
            &message,
            crate::output::color_enabled(
                crate::output::Stream::Stderr,
                false,
                crate::output::ColorMode::Auto,
            ),
        );
        let _ = writeln!(stderr, "{rendered}");
    }
    ExitCode::from(2)
}
