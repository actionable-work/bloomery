//! Production package materialization for a Bloomery workspace.
//!
//! `bloomery build` evaluates the exported `packages.<system>` derivations,
//! realizes the selected production targets in one batched Nix invocation, and
//! reports each package's output store path. An optional `--check` gate runs
//! the default check suite before realizing anything.

mod backend;

#[cfg(test)]
mod tests;

use bloomery_cli_output::{
    ColorMode, OutcomeStatus, Status, Stream, color_enabled, error_envelope, heading,
    success_envelope,
};
pub use bloomery_cli_types::BuildArgs;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

pub use backend::{NixBackend, NixCli, PackagePlan, Realization};

/// Per-package realization outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Passed,
    Failed,
    Canceled,
    NotRun,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
            Self::NotRun => "not_run",
        }
    }
}

/// Overall build outcome.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BuildStatus {
    #[default]
    Passed,
    Failed,
    Error,
    Interrupted,
}

impl BuildStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Error => "error",
            Self::Interrupted => "interrupted",
        }
    }

    fn exit_code(self) -> u8 {
        match self {
            Self::Passed => 0,
            Self::Failed => 1,
            Self::Error => 2,
            Self::Interrupted => 130,
        }
    }
}

/// One selected package and its realization evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageResult {
    pub name: String,
    pub system: String,
    pub outcome: Outcome,
    pub path: Option<String>,
    pub derivation: Option<String>,
    pub error: Option<String>,
}

/// Result of the optional `--check` gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateReport {
    pub status: BuildStatus,
    pub failed_checks: Vec<String>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Complete result of a build invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildReport {
    pub status: BuildStatus,
    pub systems: Vec<String>,
    pub packages: Vec<PackageResult>,
    pub gate: Option<GateReport>,
}

/// Failure class for a build invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildErrorKind {
    Usage,
    Operational,
}

/// A build invocation that could not produce a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildError {
    kind: BuildErrorKind,
    message: String,
}

impl BuildError {
    pub fn kind(&self) -> BuildErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn exit_code(&self) -> u8 {
        2
    }

    fn usage(message: impl Into<String>) -> Self {
        Self {
            kind: BuildErrorKind::Usage,
            message: message.into(),
        }
    }

    fn operational(message: impl Into<String>) -> Self {
        Self {
            kind: BuildErrorKind::Operational,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for BuildError {}

/// Structured outcome returned by a check gate implementation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GateOutcome {
    pub status: BuildStatus,
    pub failed_checks: Vec<String>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: u8,
}

/// Runs the default check suite as a prerequisite.
pub trait CheckGate {
    fn run(&self, root: &Path, systems: &[String]) -> GateOutcome;
}

/// Gate backed by `bloomery-check-command`'s retained default check suite.
pub struct CheckCommandGate;

impl CheckGate for CheckCommandGate {
    fn run(&self, root: &Path, systems: &[String]) -> GateOutcome {
        let args = bloomery_check_command::CheckArgs {
            operation: None,
            selectors: Vec::new(),
            systems: systems.to_vec(),
            jobs: None,
            fail_fast: false,
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let outcome =
            bloomery_check_command::run_gate_at(args, root, None, &mut stdout, &mut stderr);
        GateOutcome {
            status: match outcome.status {
                bloomery_check_command::RunStatus::Passed => BuildStatus::Passed,
                bloomery_check_command::RunStatus::Failed => BuildStatus::Failed,
                bloomery_check_command::RunStatus::Error => BuildStatus::Error,
                bloomery_check_command::RunStatus::Interrupted => BuildStatus::Interrupted,
            },
            failed_checks: outcome.failed_checks,
            stdout,
            stderr,
            exit_code: outcome.exit_code,
        }
    }
}

/// Execute `bloomery build` for the current process.
pub fn run(args: BuildArgs, json_mode: bool, color: ColorMode) -> ExitCode {
    let root = match std::env::current_dir() {
        Ok(root) => root,
        Err(error) => {
            let mut stdout = io::stdout().lock();
            let mut stderr = io::stderr().lock();
            return report_error(
                json_mode,
                color,
                &BuildError::operational(format!("unable to determine workspace root: {error}")),
                &mut stdout,
                &mut stderr,
            );
        }
    };
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    run_at(args, json_mode, color, &root, &mut stdout, &mut stderr)
}

/// Execute `bloomery build` against an injected root.
pub fn run_at(
    args: BuildArgs,
    json_mode: bool,
    color: ColorMode,
    root: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let backend = NixCli::default();
    run_at_with(
        args,
        json_mode,
        color,
        root,
        &backend,
        &CheckCommandGate,
        stdout,
        stderr,
    )
}

/// Execute `bloomery build` with injected Nix and gate backends.
#[allow(clippy::too_many_arguments)]
pub fn run_at_with<B: NixBackend, G: CheckGate>(
    args: BuildArgs,
    json_mode: bool,
    color: ColorMode,
    root: &Path,
    backend: &B,
    gate: &G,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    if !root.join("flake.nix").is_file() {
        return report_error(
            json_mode,
            color,
            &BuildError::usage(bloomery_cli_output::MISSING_FLAKE_MESSAGE),
            stdout,
            stderr,
        );
    }
    match execute(args, root, backend, gate) {
        Ok(report) => {
            let status = report.status;
            if json_mode {
                let bytes = serde_json::to_vec_pretty(&report_json(&report))
                    .expect("build report JSON is serializable");
                if stdout
                    .write_all(&bytes)
                    .and_then(|()| stdout.flush())
                    .is_err()
                {
                    let _ = writeln!(stderr, "bloomery: unable to write build result");
                    return ExitCode::from(2);
                }
            } else if write_text_report(&report, color, stdout, stderr).is_err() {
                let _ = writeln!(stderr, "bloomery: unable to write build result");
                return ExitCode::from(2);
            }
            ExitCode::from(status.exit_code())
        }
        Err(error) => report_error(json_mode, color, &error, stdout, stderr),
    }
}

fn report_error(
    json_mode: bool,
    color: ColorMode,
    error: &BuildError,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    if json_mode {
        let document = match error.kind() {
            BuildErrorKind::Usage => {
                bloomery_cli_output::usage_error_json(Some("build"), error.message())
            }
            BuildErrorKind::Operational => build_failure_json(error.message()),
        };
        match serde_json::to_vec_pretty(&document) {
            Ok(bytes) => {
                if stdout.write_all(&bytes).is_err() {
                    let _ = writeln!(stderr, "bloomery: unable to render JSON build error");
                    return ExitCode::from(2);
                }
            }
            Err(_) => {
                let _ = writeln!(stderr, "bloomery: unable to render JSON build error");
                return ExitCode::from(2);
            }
        }
    } else {
        let message = format!("error: {error}");
        let rendered = bloomery_cli_output::colorize_error(
            &message,
            color_enabled(Stream::Stderr, false, color),
        );
        let _ = writeln!(stderr, "{rendered}");
    }
    ExitCode::from(error.exit_code())
}

/// Run selection and realization, returning a report or a usage/operational error.
pub fn execute<B: NixBackend, G: CheckGate>(
    args: BuildArgs,
    root: &Path,
    backend: &B,
    gate: &G,
) -> Result<BuildReport, BuildError> {
    validate_args(&args)?;
    let systems = resolve_systems(&args, root, backend)?;

    let mut gate_report = None;
    if args.check {
        let outcome = gate.run(root, &systems);
        gate_report = Some(GateReport {
            status: outcome.status,
            failed_checks: outcome.failed_checks,
            stdout: outcome.stdout,
            stderr: outcome.stderr,
        });
        if outcome.status != BuildStatus::Passed {
            return Ok(BuildReport {
                status: outcome.status,
                systems,
                packages: Vec::new(),
                gate: gate_report,
            });
        }
    }

    let mut plans = Vec::new();
    for system in &systems {
        let mut planned = backend
            .plan_packages(root, system)
            .map_err(BuildError::operational)?;
        plans.append(&mut planned);
    }

    let selected = select_packages(&plans, &args.packages)?;
    if selected.is_empty() {
        return Err(BuildError::usage("build selection contains no packages"));
    }

    let derivations = selected
        .iter()
        .filter_map(|plan| plan.derivation.clone())
        .collect::<Vec<_>>();
    let realization = if derivations.is_empty() {
        Realization::default()
    } else {
        backend.realize(root, &derivations)
    };
    if let Some(error) = &realization.operational_error {
        return Err(BuildError::operational(error.clone()));
    }

    let outputs = if derivations.is_empty() {
        BTreeMap::new()
    } else {
        backend
            .output_paths(root, &derivations)
            .map_err(BuildError::operational)?
    };
    let all_outputs = outputs
        .values()
        .flat_map(|paths| paths.iter().cloned())
        .collect::<Vec<_>>();
    let valid = if all_outputs.is_empty() {
        BTreeSet::new()
    } else {
        backend
            .validate_outputs(root, &all_outputs)
            .map_err(BuildError::operational)?
    };

    let packages = selected
        .iter()
        .map(|plan| package_result(plan, &realization, &outputs, &valid))
        .collect::<Vec<_>>();
    let status = if !realization.canceled.is_empty() {
        BuildStatus::Interrupted
    } else if packages
        .iter()
        .any(|package| package.outcome == Outcome::Failed)
    {
        BuildStatus::Failed
    } else {
        BuildStatus::Passed
    };
    Ok(BuildReport {
        status,
        systems,
        packages,
        gate: gate_report,
    })
}

fn validate_args(args: &BuildArgs) -> Result<(), BuildError> {
    for selector in &args.packages {
        if selector.is_empty() {
            return Err(BuildError::usage("--package selectors must not be empty"));
        }
    }
    for system in &args.systems {
        if !valid_system_name(system) {
            return Err(BuildError::usage(format!(
                "invalid Nix system name '{system}'"
            )));
        }
    }
    Ok(())
}

fn resolve_systems<B: NixBackend>(
    args: &BuildArgs,
    root: &Path,
    backend: &B,
) -> Result<Vec<String>, BuildError> {
    let mut systems = Vec::new();
    for system in &args.systems {
        if !systems.contains(system) {
            systems.push(system.clone());
        }
    }
    if systems.is_empty() {
        systems.push(backend.host_system(root).map_err(BuildError::operational)?);
    }
    Ok(systems)
}

fn select_packages(
    plans: &[PackagePlan],
    selectors: &[String],
) -> Result<Vec<PackagePlan>, BuildError> {
    if selectors.is_empty() {
        return Ok(plans.to_vec());
    }
    let mut selected = Vec::new();
    for plan in plans {
        if selectors
            .iter()
            .any(|selector| glob_matches(selector, &plan.name))
            && !selected.iter().any(|chosen: &PackagePlan| {
                chosen.system == plan.system && chosen.name == plan.name
            })
        {
            selected.push(plan.clone());
        }
    }
    for selector in selectors {
        if !plans.iter().any(|plan| glob_matches(selector, &plan.name)) {
            return Err(BuildError::usage(format!(
                "package selector '{selector}' matches no package"
            )));
        }
    }
    Ok(selected)
}

fn package_result(
    plan: &PackagePlan,
    realization: &Realization,
    outputs: &BTreeMap<String, Vec<String>>,
    valid: &BTreeSet<String>,
) -> PackageResult {
    let derivation = plan.derivation.clone();
    let failure = derivation
        .as_ref()
        .and_then(|path| realization.failed.get(path));
    let path = derivation
        .as_ref()
        .and_then(|path| outputs.get(path))
        .and_then(|paths| paths.iter().find(|output| valid.contains(*output)))
        .cloned();
    let (outcome, error) = if let Some(eval_error) = &plan.eval_error {
        (Outcome::Failed, Some(eval_error.clone()))
    } else if derivation.is_none() {
        (
            Outcome::Failed,
            Some(format!("{} did not evaluate to a derivation", plan.name)),
        )
    } else if let Some(message) = failure {
        (Outcome::Failed, Some(message.clone()))
    } else if derivation
        .as_ref()
        .is_some_and(|path| realization.canceled.contains(path))
    {
        (Outcome::Canceled, None)
    } else if derivation
        .as_ref()
        .is_some_and(|path| realization.not_run.contains(path))
    {
        (Outcome::NotRun, None)
    } else if path.is_some() {
        (Outcome::Passed, None)
    } else {
        (
            Outcome::Failed,
            Some("package outputs are absent".to_owned()),
        )
    };
    PackageResult {
        name: plan.name.clone(),
        system: plan.system.clone(),
        outcome,
        path,
        derivation,
        error,
    }
}

fn outcome_status(outcome: Outcome) -> OutcomeStatus {
    match outcome {
        Outcome::Passed => OutcomeStatus::Passed,
        Outcome::Failed => OutcomeStatus::Failed,
        Outcome::Canceled | Outcome::NotRun => OutcomeStatus::Skipped,
    }
}

fn write_text_report(
    report: &BuildReport,
    color: ColorMode,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> io::Result<()> {
    let styled = color_enabled(Stream::Stdout, false, color);
    if let Some(gate) = &report.gate {
        if !gate.stdout.is_empty() {
            stdout.write_all(&gate.stdout)?;
        }
        if !gate.stderr.is_empty() {
            stderr.write_all(&gate.stderr)?;
        }
    }
    writeln!(stdout, "{}", heading("bloomery build", styled))?;
    for package in &report.packages {
        let detail = match &package.path {
            Some(path) => format!("{} ({}) {path}", package.name, package.system),
            None => format!("{} ({})", package.name, package.system),
        };
        writeln!(
            stdout,
            "{}",
            outcome_status(package.outcome).render(&detail, styled)
        )?;
        if let Some(error) = &package.error {
            writeln!(stderr, "  {error}")?;
        }
        if package.outcome == Outcome::Failed
            && let Some(derivation) = &package.derivation
        {
            writeln!(stderr, "  nix log {derivation}")?;
        }
    }
    let passed = report
        .packages
        .iter()
        .filter(|package| package.outcome == Outcome::Passed)
        .count();
    let failed = report
        .packages
        .iter()
        .filter(|package| package.outcome == Outcome::Failed)
        .count();
    if let Some(gate) = &report.gate {
        let summary = if gate.failed_checks.is_empty() {
            format!("checks {}", gate.status.as_str())
        } else {
            format!(
                "checks {} ({})",
                gate.status.as_str(),
                gate.failed_checks.join(", ")
            )
        };
        let status = match gate.status.as_str() {
            "passed" => OutcomeStatus::Passed,
            "interrupted" => OutcomeStatus::Skipped,
            _ => OutcomeStatus::Failed,
        };
        writeln!(stdout, "{}", status.render(&summary, styled))?;
    }
    let overall = if failed == 0 {
        OutcomeStatus::Passed
    } else {
        OutcomeStatus::Failed
    };
    writeln!(
        stdout,
        "{}",
        overall.render(&format!("build {passed} passed, {failed} failed"), styled)
    )?;
    stdout.flush()
}

fn report_json(report: &BuildReport) -> Value {
    let packages = report
        .packages
        .iter()
        .map(|package| {
            let mut value = json!({
                "name": package.name,
                "system": package.system,
                "status": package.outcome.as_str(),
            });
            if let Some(path) = &package.path {
                value["path"] = json!(path);
            }
            if let Some(derivation) = &package.derivation {
                value["derivation"] = json!(derivation);
            }
            if let Some(error) = &package.error {
                value["error"] = json!(error);
            }
            value
        })
        .collect::<Vec<_>>();
    let mut result = json!({
        "status": report.status.as_str(),
        "systems": report.systems,
        "packages": packages,
    });
    if let Some(gate) = &report.gate {
        result["checks"] = json!({
            "status": gate.status.as_str(),
            "failed": gate.failed_checks,
        });
    }
    success_envelope(
        "build",
        None,
        envelope_status(report.status),
        result,
        Vec::new(),
    )
}

fn envelope_status(status: BuildStatus) -> Status {
    match status {
        BuildStatus::Passed => Status::Succeeded,
        BuildStatus::Failed => Status::Failed,
        BuildStatus::Error => Status::Error,
        BuildStatus::Interrupted => Status::Interrupted,
    }
}

fn build_failure_json(message: &str) -> Value {
    error_envelope(
        Some("build"),
        None,
        Status::Error,
        "BuildError",
        "build",
        message,
        None,
    )
}

/// Case-sensitive anchored glob with `*` and `?`.
pub fn glob_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let value = value.chars().collect::<Vec<_>>();
    let mut matches = vec![false; value.len() + 1];
    matches[0] = true;

    for token in pattern {
        let mut next = vec![false; value.len() + 1];
        match token {
            '*' => {
                next[0] = matches[0];
                for index in 1..=value.len() {
                    next[index] = matches[index] || next[index - 1];
                }
            }
            '?' => next[1..].copy_from_slice(&matches[..value.len()]),
            literal => {
                for index in 1..=value.len() {
                    next[index] = matches[index - 1] && value[index - 1] == literal;
                }
            }
        }
        matches = next;
    }

    matches[value.len()]
}

fn valid_system_name(system: &str) -> bool {
    let Some((architecture, operating_system)) = system.split_once('-') else {
        return false;
    };
    !architecture.is_empty()
        && !operating_system.is_empty()
        && system.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}
