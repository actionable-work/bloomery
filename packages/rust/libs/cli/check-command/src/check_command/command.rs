use super::catalog::{NixBackend, NixCli};
use super::execution::run_checks;
use super::health::{run_retrieve as run_health, run_update as run_health_update};
use super::interrupt::InterruptFlag;
use super::model::{Outcome, RunStatus};
use super::retrieval::{run_details, run_failures, run_list};
use super::store;
use bloomery_cli_output::TerminalFacts;
pub use bloomery_cli_types::{CheckArgs, CheckOperation};
#[cfg(test)]
pub use bloomery_cli_types::{DetailsArgs, FailureArgs, HealthArgs, HealthUpdateArgs, ListArgs};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

pub(crate) use bloomery_check_output::render::report_error;
#[cfg(test)]
pub(crate) use bloomery_check_selection::selection::missing_check_controls;
pub(crate) use bloomery_check_selection::selection::{
    PlannedCatalog, discover_for_selection, plan_for_selection, warn_missing_check_controls,
};
pub(crate) use bloomery_check_support::context::CheckError;
pub(crate) use bloomery_check_support::validation::validate_args;

pub(super) struct CheckContext<'a> {
    pub(super) root: &'a Path,
    pub(super) json_mode: bool,
    pub(super) color: bloomery_cli_output::ColorMode,
    pub(super) no_write: bool,
    pub(super) backend: &'a dyn NixBackend,
    pub(super) interrupt: InterruptFlag,
    pub(super) cache_base: Option<&'a Path>,
    pub(super) terminal: TerminalFacts,
}

/// Execute `bloomery check` with explicit color and write-mode selection.
pub fn run_at(
    args: CheckArgs,
    json_mode: bool,
    color: bloomery_cli_output::ColorMode,
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
            color: bloomery_cli_output::ColorMode::Auto,
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
        exit_code: status.exit_code(),
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
    if !bloomery_cli_types::flake_present(context.root) {
        return report_error(
            context.json_mode,
            &CheckError {
                code: bloomery_cli_types::MISSING_FLAKE_CODE,
                message: bloomery_cli_types::MISSING_FLAKE_MESSAGE.to_owned(),
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
        Some(CheckOperation::Details(details)) => {
            run_details(details, &health_context(&context), stdout, stderr)
        }
        Some(CheckOperation::Health(health)) => {
            run_health(&health_context(&context), health, stdout, stderr)
        }
        Some(CheckOperation::HealthUpdate(update)) => {
            run_health_update(&health_context(&context), update, stdout, stderr)
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

fn health_context<'a>(
    context: &CheckContext<'a>,
) -> bloomery_check_support::context::HealthContext<'a, dyn NixBackend + 'a> {
    bloomery_check_support::context::HealthContext {
        root: context.root,
        json_mode: context.json_mode,
        color: context.color,
        backend: context.backend,
        cache_base: context.cache_base,
    }
}
