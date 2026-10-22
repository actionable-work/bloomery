use super::catalog::{BatchRealization, CheckPlan, NixBackend, NixTaskResult};
use super::command::{CheckArgs, CheckContext, CheckError, plan_for_selection, report_error};
use super::interrupt::{CancellationToken, InterruptFlag};
pub(super) use super::model::FailureDraft;
use super::model::{
    CheckRecord, FailureLocation, FailureRecord, Outcome, RunMode, RunRecord, RunSelection,
    RunStatus, select_ids,
};
use super::output;
use super::progress::{ProgressEvent, ProgressReducer, ProgressReporter, ProgressSink};
use super::scheduler::{
    FollowupSchedule, ScheduleReport, ScheduledState, WorkerTracker, run_with_followups,
};
use super::store;
use bloomery_check::AnalysisSession;
use bloomery_check_health::check_command::health_report::{self, HealthOutcome};
use bloomery_model::{
    Diagnostic, health::HealthConfig, sort_diagnostics as sort_model_diagnostics,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::Duration;

/// Implicit, non-selectable outcome for the mandatory formatter gate.
pub(super) const FORMAT_WORKSPACE_ID: &str = "format:workspace";

pub(super) fn validate_execution_selection(selected: &[String]) -> Result<(), CheckError> {
    if selected.is_empty() {
        Err(CheckError::usage("execution selection contains no checks"))
    } else {
        Ok(())
    }
}

pub(super) fn run_checks(
    args: CheckArgs,
    context: &CheckContext<'_>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let prepared = match prepare_check_run(&args, context) {
        Ok(prepared) => prepared,
        Err(error) => return report_error(context.json_mode, &error, None, stdout, stderr),
    };
    super::command::warn_missing_check_controls(context.root);
    run_prepared_checks(args, context, prepared, stdout, stderr)
}

struct PreparedCheckRun {
    planned: super::command::PlannedCatalog,
    selected: BTreeSet<String>,
    health_sources: Option<Result<Vec<bloomery_check::HealthSourceCrate>, String>>,
    health_baseline_path: Option<String>,
    clippy_policy: Option<bloomery_model::clippy::ClippySuppressionConfig>,
    selection: RunSelection,
    store: store::RunStore,
    allocation: store::RunAllocation,
    started_at: u64,
    jobs: usize,
}

fn prepare_check_run(
    args: &CheckArgs,
    context: &CheckContext<'_>,
) -> Result<PreparedCheckRun, CheckError> {
    let planned = if context.no_write {
        // No-write mode needs the native formatter derivation even for a
        // static-only partial selection, so force system catalog planning.
        let planned = plan_for_selection(context.root, &[], &args.systems, context.backend)?;
        let missing = planned
            .systems
            .iter()
            .filter(|system| {
                !planned
                    .ids
                    .contains(&format!("nix:{system}:core:treefmt-check"))
            })
            .cloned()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(CheckError::operational(format!(
                "selected Nix system catalog is missing core:treefmt-check: {}",
                missing.join(", ")
            )));
        }
        planned
    } else {
        let catalog = super::command::discover_for_selection(
            context.root,
            &args.selectors,
            &args.systems,
            context.backend,
        )?;
        super::command::PlannedCatalog {
            ids: catalog.ids,
            systems: catalog.systems,
            notices: catalog.notices,
            plans: BTreeMap::new(),
            outputs_by_derivation: BTreeMap::new(),
            dependencies: BTreeMap::new(),
        }
    };
    let (mut selected, mut selection) = prepare_selection(args, &planned)?;
    if context.no_write {
        for system in &planned.systems {
            selected.insert(format!("nix:{system}:core:treefmt-check"));
        }
        selection.selected_checks = selected.iter().cloned().collect();
    }
    let (health_sources, health_baseline_path) =
        prepare_health_sources(context.root, context.backend, &selected)?;
    let clippy_policy = if selected.contains("static:clippy-suppressions") {
        let loaded = bloomery_model::config::load_with_raw(context.root)
            .map_err(|diagnostic| CheckError::operational(diagnostic.message))?;
        Some(
            bloomery_model::clippy::ClippySuppressionConfig::from_raw(loaded.raw.as_ref())
                .map_err(|diagnostic| CheckError::operational(diagnostic.message))?,
        )
    } else {
        None
    };
    let store =
        store::RunStore::open(context.root, context.cache_base).map_err(CheckError::operational)?;
    let allocation = store.allocate().map_err(CheckError::operational)?;
    Ok(PreparedCheckRun {
        planned,
        selected,
        health_sources,
        health_baseline_path,
        clippy_policy,
        selection,
        store,
        allocation,
        started_at: store::timestamp_millis(),
        jobs: args.jobs.unwrap_or_else(default_jobs).max(1),
    })
}

fn prepare_selection(
    args: &CheckArgs,
    planned: &super::command::PlannedCatalog,
) -> Result<(BTreeSet<String>, RunSelection), CheckError> {
    let direct = select_ids(&planned.ids, &args.selectors).map_err(CheckError::usage)?;
    validate_execution_selection(&direct)?;
    let mut selected = direct.into_iter().collect::<BTreeSet<_>>();
    if selected.contains("static:traceability") {
        selected.insert("static:structure".to_owned());
    }
    validate_execution_selection(&selected.iter().cloned().collect::<Vec<_>>())?;
    let selection = RunSelection {
        selectors: args.selectors.clone(),
        systems: planned.systems.clone(),
        selected_checks: selected.iter().cloned().collect(),
        partial: !args.selectors.is_empty() || !args.systems.is_empty(),
    };
    Ok((selected, selection))
}

type PreparedHealthSources = (
    Option<Result<Vec<bloomery_check::HealthSourceCrate>, String>>,
    Option<String>,
);

fn prepare_health_sources(
    root: &Path,
    backend: &dyn NixBackend,
    selected: &BTreeSet<String>,
) -> Result<PreparedHealthSources, CheckError> {
    if !selected.contains("static:code-health") {
        return Ok((None, None));
    }
    let loaded = bloomery_model::config::load_with_raw(root)
        .map_err(|diagnostic| CheckError::operational(diagnostic.message))?;
    let config = HealthConfig::from_raw(loaded.raw.as_ref())
        .map_err(|diagnostic| CheckError::operational(diagnostic.message))?;
    let raw_json = loaded
        .raw
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|error| {
            CheckError::operational(format!(
                "unable to read health source-selection policy: {error}"
            ))
        })?;
    let projection = backend
        .host_system(root)
        .and_then(|system| backend.health_sources(root, &system));
    if let Ok(sources) = &projection {
        let manifests = sources
            .iter()
            .map(|source| source.manifest_path.clone())
            .collect::<Vec<_>>();
        config
            .validate_crate_exception_manifests(&manifests)
            .map_err(|diagnostic| CheckError::operational(diagnostic.message))?;
        health_report::validate_health_source_overlap(root, &config, &raw_json, sources)
            .map_err(CheckError::operational)?;
    }
    Ok((Some(projection), Some(config.baseline)))
}

struct CheckExecution {
    format: Option<FormatGate>,
    source: store::SourceMetadata,
    results: Result<ExecutionResults, String>,
    final_metrics: Option<String>,
}

fn run_prepared_checks(
    args: CheckArgs,
    context: &CheckContext<'_>,
    prepared: PreparedCheckRun,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let execution = execute_check_run(&args, context, &prepared, stderr);
    complete_check_run(context, prepared, execution, stdout, stderr)
}

fn execute_check_run(
    args: &CheckArgs,
    context: &CheckContext<'_>,
    prepared: &PreparedCheckRun,
    stderr: &mut impl Write,
) -> CheckExecution {
    let has_nix_checks = prepared.selected.iter().any(|id| id.starts_with("nix:"));
    let eligible = context.terminal.progress_eligible(context.json_mode);
    let mut progress = ProgressReporter::new(
        ProgressReducer::new(
            prepared.selected.len() + usize::from(!context.no_write),
            has_nix_checks,
        ),
        stderr,
        eligible,
        context.terminal.width(),
        context.terminal.progress_styled(),
    );
    progress.start();
    let format = if context.no_write {
        None
    } else {
        Some(run_format_gate(
            context.root,
            context.backend,
            &prepared.allocation,
            &context.interrupt,
            &mut progress,
        ))
    };
    let results = execute_after_formatter(args, context, prepared, format.as_ref(), &mut progress);
    let source = store::source_metadata(prepared.store.root());
    let final_metrics = eligible.then(|| progress.final_metrics());
    progress.finish();
    CheckExecution {
        format,
        source,
        results,
        final_metrics,
    }
}

fn execute_after_formatter(
    args: &CheckArgs,
    context: &CheckContext<'_>,
    prepared: &PreparedCheckRun,
    format: Option<&FormatGate>,
    progress: &mut ProgressReporter<impl Write>,
) -> Result<ExecutionResults, String> {
    if context.no_write {
        return execute_selected(SelectedExecutionInputs {
            root: context.root,
            selected: &prepared.selected,
            allocation: &prepared.allocation,
            backend: context.backend,
            plans: &prepared.planned.plans,
            outputs_by_derivation: &prepared.planned.outputs_by_derivation,
            dependencies: &prepared.planned.dependencies,
            health_sources: prepared.health_sources.clone(),
            clippy_policy: prepared.clippy_policy.clone(),
            no_write: context.no_write,
            interrupt: context.interrupt.clone(),
            jobs: prepared.jobs,
            fail_fast: args.fail_fast,
            progress,
        });
    }
    if format.is_some_and(|format| format.outcome == Outcome::Passed) {
        let planned = plan_for_selection(
            context.root,
            &args.selectors,
            &prepared.planned.systems,
            context.backend,
        )
        .map_err(|error| error.message)?;
        let missing = prepared
            .selected
            .iter()
            .filter(|id| id.starts_with("nix:") && !planned.plans.contains_key(*id))
            .cloned()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(format!(
                "selected check attributes disappeared after formatting: {}",
                missing.join(", ")
            ));
        }
        return execute_selected(SelectedExecutionInputs {
            root: context.root,
            selected: &prepared.selected,
            allocation: &prepared.allocation,
            backend: context.backend,
            plans: &planned.plans,
            outputs_by_derivation: &planned.outputs_by_derivation,
            dependencies: &planned.dependencies,
            health_sources: prepared.health_sources.clone(),
            clippy_policy: prepared.clippy_policy.clone(),
            no_write: context.no_write,
            interrupt: context.interrupt.clone(),
            jobs: prepared.jobs,
            fail_fast: args.fail_fast,
            progress,
        });
    }
    let format = format.expect("write mode always has a formatter gate");
    for id in &prepared.selected {
        progress.apply(ProgressEvent::CheckFinalized {
            id: id.clone(),
            outcome: Outcome::NotRun,
        });
    }
    Ok(not_run_results(&prepared.selected, format))
}

fn not_run_results(selected: &BTreeSet<String>, format: &FormatGate) -> ExecutionResults {
    ExecutionResults {
        outcomes: selected
            .iter()
            .map(|id| {
                (
                    id.clone(),
                    CheckRecord {
                        id: id.clone(),
                        outcome: Outcome::NotRun,
                        blocked_by: None,
                        logs: Vec::new(),
                    },
                )
            })
            .collect(),
        failures: Vec::new(),
        operational_error: format.operational_error.clone(),
        health_report: None,
    }
}

fn complete_check_run(
    context: &CheckContext<'_>,
    prepared: PreparedCheckRun,
    execution: CheckExecution,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let completed_at = store::timestamp_millis();
    let (record, operational_error, final_metrics) =
        assemble_run_record(context, &prepared, execution, completed_at);
    if let Err(message) = prepared.store.finalize(&prepared.allocation, &record) {
        return report_error(
            context.json_mode,
            &CheckError::operational(format!("unable to finalize retained run: {message}")),
            Some(&record.id),
            stdout,
            stderr,
        );
    }
    if record.status == RunStatus::Error {
        let message = operational_error.unwrap_or_else(|| {
            "check execution failed operationally; completed outcomes were retained".to_owned()
        });
        return report_error(
            context.json_mode,
            &CheckError::operational(message),
            Some(&record.id),
            stdout,
            stderr,
        );
    }
    write_run_summary(context, &record, final_metrics.as_deref(), stdout, stderr)
}

fn assemble_run_record(
    context: &CheckContext<'_>,
    prepared: &PreparedCheckRun,
    execution: CheckExecution,
    completed_at: u64,
) -> (RunRecord, Option<String>, Option<String>) {
    let CheckExecution {
        format,
        source,
        results,
        final_metrics,
    } = execution;
    match results {
        Ok(results) => assemble_completed_record(
            context,
            prepared,
            format,
            source,
            results,
            completed_at,
            final_metrics,
        ),
        Err(message) => {
            let outcomes = operational_outcomes(&prepared.selected, format.as_ref());
            let record = RunRecord {
                id: prepared.allocation.id.clone(),
                root: prepared.store.root().to_string_lossy().into_owned(),
                started_at: prepared.started_at,
                completed_at,
                source_revision: source.revision,
                source_dirty: source.dirty,
                status: RunStatus::Error,
                mode: run_mode(context.no_write),
                health_baseline_path: prepared.health_baseline_path.clone(),
                health_baseline_refreshed: false,
                selection: prepared.selection.clone(),
                outcomes,
                failures: Vec::new(),
                notices: prepared.planned.notices.clone(),
            };
            (record, Some(message), final_metrics)
        }
    }
}

fn assemble_completed_record(
    context: &CheckContext<'_>,
    prepared: &PreparedCheckRun,
    mut format: Option<FormatGate>,
    source: store::SourceMetadata,
    mut results: ExecutionResults,
    completed_at: u64,
    final_metrics: Option<String>,
) -> (RunRecord, Option<String>, Option<String>) {
    if let Some(format) = &mut format {
        if let Some(failure) = format.failure.take() {
            results.failures.push(failure);
        }
        results.outcomes.insert(
            FORMAT_WORKSPACE_ID.to_owned(),
            CheckRecord {
                id: FORMAT_WORKSPACE_ID.to_owned(),
                outcome: format.outcome,
                blocked_by: None,
                logs: format.log.clone().into_iter().collect(),
            },
        );
    }
    let mut health_baseline_refreshed = false;
    let health_baseline_path = prepared.health_baseline_path.clone();
    let report_for_baseline = results.health_report.clone();
    let health_report_available = results.health_report.is_some();
    let mut operational_error = results.operational_error.take();
    let health_report_retained = retain_health_report(
        &prepared.allocation,
        &mut results.health_report,
        &mut operational_error,
    );
    if !context.no_write
        && prepared.selected.contains("static:code-health")
        && health_report_retained
        && format
            .as_ref()
            .is_some_and(|format| format.outcome == Outcome::Passed)
        && let Some(report) = report_for_baseline.as_ref()
    {
        match super::health::refresh_baseline(context.root, report) {
            Ok(()) => health_baseline_refreshed = true,
            Err(message) => {
                if let Some(outcome) = results.outcomes.get_mut("static:code-health") {
                    outcome.outcome = Outcome::Failed;
                }
                results.failures.push(FailureDraft {
                    check: "static:code-health".to_owned(),
                    code: "HealthBaselineWriteFailed".to_owned(),
                    subject: health_baseline_path.clone(),
                    location: None,
                    message,
                    notes: Vec::new(),
                    log: None,
                    nix_log: None,
                    focus_tail: false,
                    occurrence: usize::MAX,
                });
            }
        }
    }
    let status = run_status(
        &context.interrupt,
        operational_error.is_some(),
        &results.outcomes,
    );
    let failures = assign_failure_ids(results.failures);
    let notices = run_notices(
        prepared,
        health_report_available,
        operational_error.is_none(),
    );
    link_blocked_traceability(&mut results.outcomes, &failures);
    let record = RunRecord {
        id: prepared.allocation.id.clone(),
        root: prepared.store.root().to_string_lossy().into_owned(),
        started_at: prepared.started_at,
        completed_at,
        source_revision: source.revision,
        source_dirty: source.dirty,
        status,
        mode: run_mode(context.no_write),
        health_baseline_path,
        health_baseline_refreshed,
        selection: prepared.selection.clone(),
        outcomes: results.outcomes.into_values().collect(),
        failures,
        notices,
    };
    (record, operational_error, final_metrics)
}

fn operational_outcomes(
    selected: &BTreeSet<String>,
    format: Option<&FormatGate>,
) -> Vec<CheckRecord> {
    let mut outcomes = selected
        .iter()
        .map(|id| CheckRecord {
            id: id.clone(),
            outcome: Outcome::NotRun,
            blocked_by: None,
            logs: Vec::new(),
        })
        .collect::<Vec<_>>();
    if let Some(format) = format {
        outcomes.push(CheckRecord {
            id: FORMAT_WORKSPACE_ID.to_owned(),
            outcome: format.outcome,
            blocked_by: None,
            logs: format.log.clone().into_iter().collect(),
        });
    }
    outcomes.sort_by(|left, right| left.id.cmp(&right.id));
    outcomes
}

fn run_mode(no_write: bool) -> RunMode {
    if no_write {
        RunMode::NoWrite
    } else {
        RunMode::Write
    }
}

fn retain_health_report(
    allocation: &store::RunAllocation,
    report: &mut Option<Value>,
    operational_error: &mut Option<String>,
) -> bool {
    if let Some(report) = report.take()
        && let Err(error) = allocation.retain_health_report(&report)
    {
        operational_error.get_or_insert(error);
        return false;
    }
    true
}

fn run_status(
    interrupt: &InterruptFlag,
    has_operational_error: bool,
    outcomes: &BTreeMap<String, CheckRecord>,
) -> RunStatus {
    if interrupt.is_set() {
        RunStatus::Interrupted
    } else if has_operational_error {
        RunStatus::Error
    } else if outcomes
        .values()
        .any(|outcome| outcome.outcome != Outcome::Passed)
    {
        RunStatus::Failed
    } else {
        RunStatus::Passed
    }
}

fn run_notices(
    prepared: &PreparedCheckRun,
    health_report_available: bool,
    operational: bool,
) -> Vec<super::model::Notice> {
    let mut notices = prepared.planned.notices.clone();
    if health_report_available && operational {
        notices.push(super::model::Notice {
            code: "HealthReportRetained".to_owned(),
            message: format!(
                "retrieve the retained measurement with `bloomery check health --run {}`",
                prepared.allocation.id
            ),
        });
    }
    notices
}

fn link_blocked_traceability(
    outcomes: &mut BTreeMap<String, CheckRecord>,
    failures: &[FailureRecord],
) {
    if let Some(blocked) = outcomes.get_mut("static:traceability")
        && blocked.outcome == Outcome::Blocked
    {
        blocked.blocked_by = failures
            .iter()
            .find(|failure| failure.check == "static:structure")
            .map(|failure| failure.id.clone());
    }
}

fn write_run_summary(
    context: &CheckContext<'_>,
    record: &RunRecord,
    final_metrics: Option<&str>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let exit_code = match record.status {
        RunStatus::Passed => ExitCode::SUCCESS,
        RunStatus::Failed => ExitCode::from(1),
        RunStatus::Interrupted => ExitCode::from(130),
        RunStatus::Error => ExitCode::from(2),
    };
    let bytes = if context.json_mode {
        output::run_summary_json(record).bytes
    } else {
        crate::output::colorize_check(
            &output::run_summary_text(record, final_metrics),
            crate::output::color_enabled(crate::output::Stream::Stdout, false, context.color),
        )
        .into_bytes()
    };
    match stdout.write_all(&bytes).and_then(|()| stdout.flush()) {
        Ok(()) => exit_code,
        Err(error) => {
            let _ = writeln!(stderr, "bloomery: unable to write check result: {error}");
            ExitCode::from(2)
        }
    }
}

struct FormatGate {
    outcome: Outcome,
    failure: Option<FailureDraft>,
    operational_error: Option<String>,
    log: Option<String>,
}

/// Run the mandatory `nix fmt` preflight exactly once. The gate shares the
/// run's interruption flag but not the check scheduler's admission or
/// fail-fast behavior: a nonzero formatter exit always stops check work.
fn run_format_gate<W: Write>(
    root: &Path,
    backend: &dyn NixBackend,
    allocation: &store::RunAllocation,
    interrupt: &InterruptFlag,
    progress: &mut ProgressReporter<W>,
) -> FormatGate {
    let log_name = "format-workspace.log";
    let log_reference = format!("logs/{log_name}");
    let finalize = |progress: &mut ProgressReporter<W>, outcome: Outcome| {
        progress.apply(ProgressEvent::CheckFinalized {
            id: FORMAT_WORKSPACE_ID.to_owned(),
            outcome,
        });
    };
    let log_path = match allocation.prepare_log(log_name) {
        Ok(path) => path,
        Err(message) => {
            finalize(progress, Outcome::NotRun);
            return FormatGate {
                outcome: Outcome::NotRun,
                failure: None,
                operational_error: Some(message),
                log: None,
            };
        }
    };
    match backend.format_workspace(root, &log_path, CancellationToken::new(interrupt.clone())) {
        NixTaskResult::Succeeded(()) => {
            finalize(progress, Outcome::Passed);
            FormatGate {
                outcome: Outcome::Passed,
                failure: None,
                operational_error: None,
                log: Some(log_reference),
            }
        }
        NixTaskResult::Failed { code, message } => {
            finalize(progress, Outcome::Failed);
            FormatGate {
                outcome: Outcome::Failed,
                failure: Some(FailureDraft {
                    check: FORMAT_WORKSPACE_ID.to_owned(),
                    code,
                    subject: None,
                    location: None,
                    message,
                    notes: Vec::new(),
                    log: Some(log_reference.clone()),
                    nix_log: None,
                    focus_tail: true,
                    occurrence: 0,
                }),
                operational_error: None,
                log: Some(log_reference),
            }
        }
        NixTaskResult::Canceled => {
            finalize(progress, Outcome::Canceled);
            FormatGate {
                outcome: Outcome::Canceled,
                failure: None,
                operational_error: None,
                log: Some(log_reference),
            }
        }
        NixTaskResult::OperationalError(message) => {
            finalize(progress, Outcome::NotRun);
            FormatGate {
                outcome: Outcome::NotRun,
                failure: None,
                operational_error: Some(message),
                log: Some(log_reference),
            }
        }
    }
}

pub(super) fn default_jobs() -> usize {
    std::thread::available_parallelism()
        .map_or(1, |count| count.get())
        .max(1)
}

struct ExecutionResults {
    outcomes: BTreeMap<String, CheckRecord>,
    failures: Vec<FailureDraft>,
    operational_error: Option<String>,
    health_report: Option<Value>,
}

type InitialWork<'a> = (
    String,
    Box<dyn FnOnce(CancellationToken) -> InitialTask + Send + 'a>,
);

struct InitialTask {
    result: InitialTaskResult,
}

enum InitialTaskResult {
    Structure(Result<Option<Box<bloomery_model::Context>>, Vec<Diagnostic>>),
    Traceability(Result<(), Vec<Diagnostic>>),
    Health(Result<HealthOutcome, Vec<Diagnostic>>),
    HealthCanceled,
    Clippy(Result<(), Vec<Diagnostic>>),
    ClippyCanceled,
    SharedAnalysis {
        traceability: Result<(), Vec<Diagnostic>>,
        health: Result<HealthOutcome, Vec<Diagnostic>>,
    },
    NixBatch {
        log: String,
        result: BatchRealization,
    },
}

enum InitialState {
    StaticPassed,
    StaticFailed(Vec<Diagnostic>),
    HealthPassed(HealthOutcome),
    HealthFailed(HealthOutcome),
    HealthDiagnosticFailed(Vec<Diagnostic>),
    HealthCanceled,
    ClippyCanceled,
    Canceled,
    NotRun,
}

fn initial_outcomes(selected: &BTreeSet<String>) -> BTreeMap<String, CheckRecord> {
    selected
        .iter()
        .map(|id| {
            (
                id.clone(),
                CheckRecord {
                    id: id.clone(),
                    outcome: Outcome::NotRun,
                    blocked_by: None,
                    logs: Vec::new(),
                },
            )
        })
        .collect()
}

struct InitialTaskInputs<'a> {
    root: &'a Path,
    selected: &'a BTreeSet<String>,
    allocation: &'a store::RunAllocation,
    backend: &'a dyn NixBackend,
    batch_plans: Vec<CheckPlan>,
    batch_derivations: Vec<String>,
    batch_outputs: BTreeMap<String, Vec<String>>,
    health_sources: Option<Result<Vec<bloomery_check::HealthSourceCrate>, String>>,
    clippy_policy: Option<bloomery_model::clippy::ClippySuppressionConfig>,
    no_write: bool,
    fail_fast: bool,
    sink: &'a ProgressSink,
}

fn build_initial_tasks<'a>(inputs: InitialTaskInputs<'a>) -> Vec<InitialWork<'a>> {
    let InitialTaskInputs {
        root,
        selected,
        allocation,
        backend,
        batch_plans,
        batch_derivations,
        batch_outputs,
        health_sources,
        clippy_policy,
        no_write,
        fail_fast,
        sink,
    } = inputs;
    let mut tasks = Vec::new();
    if selected.contains("static:clippy-suppressions") {
        let root = root.to_path_buf();
        let allow = clippy_policy.unwrap_or_default().allow;
        tasks.push((
            "static:clippy-suppressions".to_owned(),
            Box::new(move |cancellation: CancellationToken| {
                let result = if cancellation.is_canceled() {
                    InitialTaskResult::ClippyCanceled
                } else {
                    InitialTaskResult::Clippy(run_clippy_suppression_check(&root, &allow))
                };
                InitialTask { result }
            }) as _,
        ));
    }
    if selected.contains("static:structure") {
        let root = root.to_path_buf();
        tasks.push((
            "static:structure".to_owned(),
            Box::new(move |_cancellation: CancellationToken| InitialTask {
                result: InitialTaskResult::Structure(
                    bloomery_workspace::load(&root).map(|context| Some(Box::new(context))),
                ),
            }) as _,
        ));
    }
    if selected.contains("static:code-health") && !selected.contains("static:traceability") {
        let root = root.to_path_buf();
        let sources = health_sources
            .clone()
            .unwrap_or_else(|| Err("health source projection was not prepared".to_owned()));
        tasks.push((
            "static:code-health".to_owned(),
            Box::new(move |cancellation: CancellationToken| InitialTask {
                result: if cancellation.is_canceled() {
                    InitialTaskResult::HealthCanceled
                } else {
                    let analysis = AnalysisSession::new();
                    InitialTaskResult::Health(health_report::collect_health_report(
                        &root, &sources, &analysis, no_write,
                    ))
                },
            }) as _,
        ));
    }
    if !batch_plans.is_empty() {
        tasks.push(batch_task(BatchTaskInputs {
            root,
            allocation,
            backend,
            batch_plans,
            batch_derivations,
            batch_outputs,
            fail_fast,
            sink,
        }));
    }
    tasks
}

fn selected_derivations(plans: &[CheckPlan]) -> Vec<String> {
    plans
        .iter()
        .filter_map(|plan| plan.derivation.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

struct BatchTaskInputs<'a> {
    root: &'a Path,
    allocation: &'a store::RunAllocation,
    backend: &'a dyn NixBackend,
    batch_plans: Vec<CheckPlan>,
    batch_derivations: Vec<String>,
    batch_outputs: BTreeMap<String, Vec<String>>,
    fail_fast: bool,
    sink: &'a ProgressSink,
}

fn batch_task<'a>(inputs: BatchTaskInputs<'a>) -> InitialWork<'a> {
    let BatchTaskInputs {
        root,
        allocation,
        backend,
        batch_plans,
        batch_derivations,
        batch_outputs,
        fail_fast,
        sink,
    } = inputs;
    let root = root.to_path_buf();
    let allocation = allocation.clone();
    let sink = sink.clone();
    (
        "nix:batch".to_owned(),
        Box::new(move |cancellation: CancellationToken| {
            sink.emit(ProgressEvent::DerivationMetadataAvailable);
            for derivation in &batch_derivations {
                sink.work(super::progress::DerivationEvent::Discovered {
                    drv: derivation.clone(),
                });
            }
            if sink.is_enabled() {
                let output_metadata_complete = batch_outputs.len() == batch_derivations.len()
                    && batch_outputs.values().all(|outputs| !outputs.is_empty());
                if !output_metadata_complete {
                    sink.emit(ProgressEvent::DerivationMetricsIncomplete);
                } else {
                    let outputs = batch_outputs
                        .values()
                        .flatten()
                        .cloned()
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect::<Vec<_>>();
                    match backend.validate_outputs(&root, &outputs) {
                        Ok(valid) => {
                            for (derivation, required) in &batch_outputs {
                                if !required.is_empty()
                                    && required.iter().all(|output| valid.contains(output))
                                {
                                    sink.work(super::progress::DerivationEvent::StoreReused {
                                        drv: derivation.clone(),
                                    });
                                    sink.work(super::progress::DerivationEvent::OutputsValid {
                                        drv: derivation.clone(),
                                    });
                                }
                            }
                        }
                        Err(_) => sink.emit(ProgressEvent::DerivationMetricsIncomplete),
                    }
                }
            }
            let log_name = "nix-batch.log";
            let result = match allocation.prepare_log(log_name) {
                Ok(log_path) => realize_observing_checks(
                    backend,
                    &root,
                    &batch_plans,
                    fail_fast,
                    &log_path,
                    cancellation,
                    &sink,
                ),
                Err(message) => BatchRealization {
                    operational_error: Some(message),
                    ..BatchRealization::default()
                },
            };
            InitialTask {
                result: InitialTaskResult::NixBatch {
                    log: format!("logs/{log_name}"),
                    result,
                },
            }
        }),
    )
}

fn publish_valid_checks(plans: &[CheckPlan], valid: &BTreeSet<String>, sink: &ProgressSink) {
    for plan in plans {
        if !plan.outputs.is_empty()
            && plan.outputs.iter().all(|path| valid.contains(path))
            && let Some(drv) = &plan.derivation
        {
            sink.work(super::progress::DerivationEvent::OutputsValid { drv: drv.clone() });
        }
    }
}

fn observe_pending_checks(
    backend: &dyn NixBackend,
    root: &Path,
    pending: &mut Vec<CheckPlan>,
    sink: &ProgressSink,
) {
    let outputs = pending
        .iter()
        .flat_map(|plan| plan.outputs.iter().cloned())
        .collect::<Vec<_>>();
    if outputs.is_empty() {
        return;
    }
    if let Ok(valid) = backend.validate_outputs(root, &outputs) {
        publish_valid_checks(pending, &valid, sink);
        pending.retain(|plan| !plan.outputs.iter().all(|path| valid.contains(path)));
    }
}

/// Observe only selected check outputs while Nix owns dependency scheduling.
/// Polling runs alongside realization and stops as soon as the batch returns.
fn realize_observing_checks(
    backend: &dyn NixBackend,
    root: &Path,
    plans: &[CheckPlan],
    fail_fast: bool,
    log_path: &Path,
    cancellation: CancellationToken,
    sink: &ProgressSink,
) -> BatchRealization {
    if !sink.is_enabled() {
        return backend.realize_batch(root, plans, fail_fast, log_path, cancellation, sink);
    }
    std::thread::scope(|scope| {
        let (sender, receiver) = mpsc::channel();
        scope.spawn(move || {
            let result =
                backend.realize_batch(root, plans, fail_fast, log_path, cancellation, sink);
            let _ = sender.send(result);
        });
        let mut pending = plans.to_vec();
        loop {
            match receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(result) => return result,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return BatchRealization {
                        operational_error: Some("Nix realization worker disconnected".to_owned()),
                        ..BatchRealization::default()
                    };
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    observe_pending_checks(backend, root, &mut pending, sink);
                }
            }
        }
    })
}

struct SelectedExecutionInputs<'a, W: Write> {
    root: &'a Path,
    selected: &'a BTreeSet<String>,
    allocation: &'a store::RunAllocation,
    backend: &'a dyn NixBackend,
    plans: &'a BTreeMap<String, CheckPlan>,
    outputs_by_derivation: &'a BTreeMap<String, Vec<String>>,
    dependencies: &'a BTreeMap<String, Vec<String>>,
    health_sources: Option<Result<Vec<bloomery_check::HealthSourceCrate>, String>>,
    clippy_policy: Option<bloomery_model::clippy::ClippySuppressionConfig>,
    no_write: bool,
    interrupt: InterruptFlag,
    jobs: usize,
    fail_fast: bool,
    progress: &'a mut ProgressReporter<W>,
}

fn execute_selected<W: Write>(
    inputs: SelectedExecutionInputs<'_, W>,
) -> Result<ExecutionResults, String> {
    let SelectedExecutionInputs {
        root,
        selected,
        allocation,
        backend,
        plans,
        outputs_by_derivation,
        dependencies,
        health_sources,
        clippy_policy,
        no_write,
        interrupt,
        jobs,
        fail_fast,
        progress,
    } = inputs;
    let mut outcomes = initial_outcomes(selected);
    let nix_checks = selected
        .iter()
        .filter(|id| id.starts_with("nix:"))
        .cloned()
        .collect::<Vec<_>>();
    let batch_plans = nix_checks
        .iter()
        .filter_map(|id| plans.get(id).cloned())
        .collect::<Vec<_>>();
    let batch_derivations = selected_derivations(&batch_plans);
    let batch_outputs = batch_plans
        .iter()
        .filter_map(|plan| {
            plan.derivation
                .as_ref()
                .map(|drv| (drv.clone(), plan.outputs.clone()))
        })
        .collect::<BTreeMap<_, _>>();

    let (event_sender, event_receiver) = mpsc::channel();
    let mut checks = BTreeMap::<String, Vec<String>>::new();
    for id in &nix_checks {
        if let Some(drv) = plans.get(id).and_then(|plan| plan.derivation.as_ref()) {
            checks.entry(drv.clone()).or_default().push(id.clone());
        }
    }
    let output_metadata = outputs_by_derivation
        .iter()
        .flat_map(|(drv, paths)| paths.iter().map(move |path| (path.clone(), drv.clone())))
        .collect();
    let sink = ProgressSink::new(event_sender, progress.is_enabled())
        .with_metadata(checks, output_metadata);
    let initial_tasks = build_initial_tasks(InitialTaskInputs {
        root,
        selected,
        allocation,
        backend,
        batch_plans: batch_plans.clone(),
        batch_derivations,
        batch_outputs,
        health_sources: health_sources.clone(),
        clippy_policy,
        no_write,
        fail_fast,
        sink: &sink,
    });
    let initial_report = execute_initial_schedule(
        initial_tasks,
        InitialScheduleInputs {
            root,
            selected,
            health_sources: health_sources.as_ref(),
            no_write,
            jobs,
            interrupt,
            fail_fast,
            sink: &sink,
            event_receiver: &event_receiver,
            progress,
        },
    );
    let mut reduction = reduce_initial_report(initial_report);
    let mut failures = Vec::new();
    let mut operational_error = reduction.batch.operational_error.clone();
    let mut health_report = None;
    reduction.batch_canceled |= reduction.batch.canceled;
    let valid =
        validate_selected_outputs(root, &nix_checks, plans, backend, &mut operational_error);
    publish_valid_checks(&batch_plans, &valid, &sink);
    apply_initial_states(
        root,
        reduction.states,
        &mut outcomes,
        &mut failures,
        &mut health_report,
    );
    let mut dependencies = dependencies.clone();
    dependencies.extend(reduction.batch.dependencies.clone());
    let nix_inputs = NixOutcomeInputs {
        plans,
        dependencies: &dependencies,
        batch: &reduction.batch,
        batch_log: &reduction.batch_log,
        batch_canceled: reduction.batch_canceled,
        valid: &valid,
    };
    apply_nix_outcomes(&nix_checks, &nix_inputs, &mut outcomes, &mut failures);

    finalize_selected_outcomes(selected, &mut outcomes, &sink, &event_receiver, progress);

    Ok(ExecutionResults {
        outcomes,
        failures,
        operational_error,
        health_report,
    })
}

fn finalize_selected_outcomes<W: Write>(
    selected: &BTreeSet<String>,
    outcomes: &mut BTreeMap<String, CheckRecord>,
    sink: &ProgressSink,
    event_receiver: &mpsc::Receiver<ProgressEvent>,
    progress: &mut ProgressReporter<W>,
) {
    if selected.contains("static:traceability")
        && outcomes
            .get("static:structure")
            .is_some_and(|structure| structure.outcome == Outcome::Failed)
    {
        set_outcome(outcomes, "static:traceability", Outcome::Blocked);
    }
    ensure_selected_outcomes(selected, outcomes);
    for outcome in outcomes.values_mut() {
        outcome.logs.sort();
        outcome.logs.dedup();
    }
    for (id, record) in outcomes.iter() {
        sink.emit(ProgressEvent::CheckFinalized {
            id: id.clone(),
            outcome: record.outcome,
        });
    }
    while let Ok(event) = event_receiver.try_recv() {
        progress.apply(event);
    }
    progress.refresh();
}

fn ensure_selected_outcomes(
    selected: &BTreeSet<String>,
    outcomes: &mut BTreeMap<String, CheckRecord>,
) {
    for check in selected {
        outcomes
            .entry(check.clone())
            .or_insert_with(|| CheckRecord {
                id: check.clone(),
                outcome: Outcome::NotRun,
                blocked_by: None,
                logs: Vec::new(),
            });
    }
}

fn completed_outcome(task: &InitialTask) -> Option<Outcome> {
    match &task.result {
        InitialTaskResult::Structure(Ok(_)) | InitialTaskResult::Traceability(Ok(())) => {
            Some(Outcome::Passed)
        }
        InitialTaskResult::Structure(Err(_)) | InitialTaskResult::Traceability(Err(_)) => {
            Some(Outcome::Failed)
        }
        InitialTaskResult::Health(Ok(health)) => Some(if health.failures.is_empty() {
            Outcome::Passed
        } else {
            Outcome::Failed
        }),
        InitialTaskResult::Health(Err(_)) => Some(Outcome::Failed),
        InitialTaskResult::HealthCanceled => Some(Outcome::Canceled),
        InitialTaskResult::Clippy(Ok(())) => Some(Outcome::Passed),
        InitialTaskResult::Clippy(Err(_)) => Some(Outcome::Failed),
        InitialTaskResult::ClippyCanceled => Some(Outcome::Canceled),
        InitialTaskResult::SharedAnalysis { .. } => None,
        InitialTaskResult::NixBatch { .. } => None,
    }
}

struct InitialScheduleInputs<'a, W: Write> {
    root: &'a Path,
    selected: &'a BTreeSet<String>,
    health_sources: Option<&'a Result<Vec<bloomery_check::HealthSourceCrate>, String>>,
    no_write: bool,
    jobs: usize,
    interrupt: InterruptFlag,
    fail_fast: bool,
    sink: &'a ProgressSink,
    event_receiver: &'a mpsc::Receiver<ProgressEvent>,
    progress: &'a mut ProgressReporter<W>,
}

fn execute_initial_schedule<'a, W: Write>(
    tasks: Vec<InitialWork<'a>>,
    inputs: InitialScheduleInputs<'a, W>,
) -> ScheduleReport<InitialTask> {
    let InitialScheduleInputs {
        root,
        selected,
        health_sources,
        no_write,
        jobs,
        interrupt,
        fail_fast,
        sink,
        event_receiver,
        progress,
    } = inputs;
    run_with_followups(FollowupSchedule {
        tasks,
        jobs,
        interrupt,
        should_stop: Box::new(|task: &InitialTask| initial_task_stops_admission(task, fail_fast)),
        followups: Box::new(|task: &mut InitialTask| {
            followup_work(root, selected, health_sources, no_write, task)
        }),
        on_complete: Box::new(|id: &str, task: &InitialTask| {
            publish_completion_events(id, task, selected, sink)
        }),
        on_stopped: Box::new(|queued: &[String]| publish_not_run_events(queued, sink)),
        on_spawned: Box::new(|tracker: &WorkerTracker| {
            drain_progress_events(tracker, event_receiver, progress)
        }),
    })
}

fn followup_work<'a>(
    root: &'a Path,
    selected: &BTreeSet<String>,
    health_sources: Option<&Result<Vec<bloomery_check::HealthSourceCrate>, String>>,
    no_write: bool,
    task: &mut InitialTask,
) -> Vec<InitialWork<'a>> {
    if let InitialTaskResult::Structure(Ok(context)) = &mut task.result
        && let Some(context) = context.take()
        && selected.contains("static:traceability")
    {
        return vec![shared_analysis_task(
            root,
            *context,
            selected.contains("static:code-health"),
            health_sources,
            no_write,
        )];
    }
    if matches!(&task.result, InitialTaskResult::Structure(Err(_)))
        && selected.contains("static:code-health")
        && selected.contains("static:traceability")
    {
        return vec![standalone_health_task(root, health_sources, no_write)];
    }
    Vec::new()
}

fn shared_analysis_task<'a>(
    root: &'a Path,
    context: bloomery_model::Context,
    health_selected: bool,
    health_sources: Option<&Result<Vec<bloomery_check::HealthSourceCrate>, String>>,
    no_write: bool,
) -> InitialWork<'a> {
    let traceability_selected = true;
    let sources = health_sources
        .cloned()
        .unwrap_or_else(|| Err("health source projection was not prepared".to_owned()));
    let id = if health_selected {
        "static:health-traceability"
    } else {
        "static:traceability"
    };
    (
        id.to_owned(),
        Box::new(move |cancellation: CancellationToken| {
            let analysis = AnalysisSession::new();
            let traceability = traceability_selected
                .then(|| bloomery_check::run_with_session(&context, &analysis));
            let health = health_selected.then(|| {
                if cancellation.is_canceled() {
                    Err(vec![Diagnostic::new(
                        "HealthCanceled",
                        "code health analysis was canceled before it started",
                    )])
                } else {
                    health_report::collect_health_report(root, &sources, &analysis, no_write)
                }
            });
            let result = match (traceability, health) {
                (Some(traceability), Some(health)) => InitialTaskResult::SharedAnalysis {
                    traceability,
                    health,
                },
                (Some(traceability), None) => InitialTaskResult::Traceability(traceability),
                (None, Some(health)) => InitialTaskResult::Health(health),
                (None, None) => unreachable!("follow-up has a consumer"),
            };
            InitialTask { result }
        }),
    )
}

fn standalone_health_task<'a>(
    root: &'a Path,
    health_sources: Option<&Result<Vec<bloomery_check::HealthSourceCrate>, String>>,
    no_write: bool,
) -> InitialWork<'a> {
    let root = root.to_path_buf();
    let sources = health_sources
        .cloned()
        .unwrap_or_else(|| Err("health source projection was not prepared".to_owned()));
    (
        "static:code-health".to_owned(),
        Box::new(move |cancellation: CancellationToken| {
            let result = if cancellation.is_canceled() {
                InitialTaskResult::HealthCanceled
            } else {
                let analysis = AnalysisSession::new();
                InitialTaskResult::Health(health_report::collect_health_report(
                    &root, &sources, &analysis, no_write,
                ))
            };
            InitialTask { result }
        }),
    )
}

fn publish_completion_events(
    id: &str,
    task: &InitialTask,
    selected: &BTreeSet<String>,
    sink: &ProgressSink,
) {
    if let Some(outcome) = completed_outcome(task) {
        sink.emit(ProgressEvent::CheckFinalized {
            id: id.to_owned(),
            outcome,
        });
    }
    if let InitialTaskResult::SharedAnalysis {
        traceability,
        health,
    } = &task.result
    {
        publish_shared_outcomes(traceability, health, sink);
    }
    if id == "static:structure"
        && matches!(&task.result, InitialTaskResult::Structure(Err(_)))
        && selected.contains("static:traceability")
    {
        sink.emit(ProgressEvent::CheckFinalized {
            id: "static:traceability".to_owned(),
            outcome: Outcome::Blocked,
        });
    }
}

fn publish_shared_outcomes(
    traceability: &Result<(), Vec<Diagnostic>>,
    health: &Result<HealthOutcome, Vec<Diagnostic>>,
    sink: &ProgressSink,
) {
    let traceability_outcome = if traceability.is_ok() {
        Outcome::Passed
    } else {
        Outcome::Failed
    };
    let health_outcome = match health {
        Ok(health) if health.failures.is_empty() => Outcome::Passed,
        Ok(_) | Err(_) => Outcome::Failed,
    };
    for (id, outcome) in [
        ("static:traceability", traceability_outcome),
        ("static:code-health", health_outcome),
    ] {
        sink.emit(ProgressEvent::CheckFinalized {
            id: id.to_owned(),
            outcome,
        });
    }
}

fn publish_not_run_events(queued: &[String], sink: &ProgressSink) {
    for id in queued {
        sink.emit(ProgressEvent::CheckFinalized {
            id: id.clone(),
            outcome: Outcome::NotRun,
        });
    }
}

fn drain_progress_events<W: Write>(
    tracker: &WorkerTracker,
    event_receiver: &mpsc::Receiver<ProgressEvent>,
    progress: &mut ProgressReporter<W>,
) {
    while !tracker.wait_timeout(Duration::from_millis(5)) {
        while let Ok(event) = event_receiver.try_recv() {
            progress.apply(event);
        }
        progress.refresh();
    }
    while let Ok(event) = event_receiver.try_recv() {
        progress.apply(event);
    }
    progress.refresh();
}

fn initial_task_stops_admission(task: &InitialTask, fail_fast: bool) -> bool {
    match &task.result {
        InitialTaskResult::Structure(Err(_))
        | InitialTaskResult::Traceability(Err(_))
        | InitialTaskResult::Health(Err(_))
        | InitialTaskResult::Clippy(Err(_)) => fail_fast,
        InitialTaskResult::Health(Ok(health)) => fail_fast && !health.failures.is_empty(),
        InitialTaskResult::NixBatch { result, .. } => {
            result.operational_error.is_some() || (fail_fast && !result.failures.is_empty())
        }
        InitialTaskResult::Structure(Ok(_))
        | InitialTaskResult::Traceability(Ok(()))
        | InitialTaskResult::HealthCanceled
        | InitialTaskResult::Clippy(Ok(()))
        | InitialTaskResult::ClippyCanceled => false,
        InitialTaskResult::SharedAnalysis {
            traceability,
            health,
        } => {
            fail_fast
                && (traceability.is_err()
                    || health.is_err()
                    || health
                        .as_ref()
                        .is_ok_and(|health| !health.failures.is_empty()))
        }
    }
}

fn static_state(result: Result<(), Vec<Diagnostic>>) -> InitialState {
    match result {
        Ok(()) => InitialState::StaticPassed,
        Err(mut diagnostics) => {
            sort_model_diagnostics(&mut diagnostics);
            InitialState::StaticFailed(diagnostics)
        }
    }
}

fn health_state(result: Result<HealthOutcome, Vec<Diagnostic>>) -> InitialState {
    match result {
        Ok(health) if health.failures.is_empty() => InitialState::HealthPassed(health),
        Ok(health) => InitialState::HealthFailed(health),
        Err(mut diagnostics) => {
            sort_model_diagnostics(&mut diagnostics);
            InitialState::HealthDiagnosticFailed(diagnostics)
        }
    }
}

struct InitialReduction {
    states: BTreeMap<String, InitialState>,
    batch_log: Option<String>,
    batch: BatchRealization,
    batch_canceled: bool,
}

fn reduce_initial_report(report: ScheduleReport<InitialTask>) -> InitialReduction {
    let mut reduction = InitialReduction {
        states: BTreeMap::new(),
        batch_log: None,
        batch: BatchRealization::default(),
        batch_canceled: false,
    };
    for result in report.results {
        match result.id.as_str() {
            "nix:batch" => reduce_batch_result(result.state, &mut reduction),
            "static:health-traceability" => {
                reduce_shared_result(result.state, &mut reduction.states);
            }
            _ => {
                if let Some(state) = reduce_task_state(result.state) {
                    reduction.states.insert(result.id, state);
                }
            }
        }
    }
    reduction
}

fn reduce_batch_result(state: ScheduledState<InitialTask>, reduction: &mut InitialReduction) {
    match state {
        ScheduledState::Completed(task) => {
            if let InitialTaskResult::NixBatch { log, result } = task.result {
                reduction.batch_log = Some(log);
                reduction.batch = result;
            }
        }
        ScheduledState::Canceled => reduction.batch_canceled = true,
        ScheduledState::NotRun => {}
    }
}

fn reduce_shared_result(
    state: ScheduledState<InitialTask>,
    states: &mut BTreeMap<String, InitialState>,
) {
    match state {
        ScheduledState::Completed(task) => {
            if let InitialTaskResult::SharedAnalysis {
                traceability,
                health,
            } = task.result
            {
                states.insert("static:traceability".to_owned(), static_state(traceability));
                states.insert("static:code-health".to_owned(), health_state(health));
            }
        }
        ScheduledState::Canceled => {
            states.insert("static:traceability".to_owned(), InitialState::Canceled);
            states.insert(
                "static:code-health".to_owned(),
                InitialState::HealthCanceled,
            );
        }
        ScheduledState::NotRun => {
            states.insert("static:traceability".to_owned(), InitialState::NotRun);
            states.insert("static:code-health".to_owned(), InitialState::NotRun);
        }
    }
}

fn reduce_task_state(state: ScheduledState<InitialTask>) -> Option<InitialState> {
    match state {
        ScheduledState::Completed(task) => match task.result {
            InitialTaskResult::Structure(Ok(_)) | InitialTaskResult::Traceability(Ok(())) => {
                Some(InitialState::StaticPassed)
            }
            InitialTaskResult::Structure(Err(diagnostics))
            | InitialTaskResult::Traceability(Err(diagnostics)) => {
                Some(static_state(Err(diagnostics)))
            }
            InitialTaskResult::Health(result) => Some(health_state(result)),
            InitialTaskResult::Clippy(Ok(())) => Some(InitialState::StaticPassed),
            InitialTaskResult::Clippy(Err(diagnostics)) => Some(static_state(Err(diagnostics))),
            InitialTaskResult::HealthCanceled => Some(InitialState::HealthCanceled),
            InitialTaskResult::ClippyCanceled => Some(InitialState::ClippyCanceled),
            InitialTaskResult::SharedAnalysis { .. } | InitialTaskResult::NixBatch { .. } => None,
        },
        ScheduledState::Canceled => Some(InitialState::Canceled),
        ScheduledState::NotRun => Some(InitialState::NotRun),
    }
}

fn validate_selected_outputs(
    root: &Path,
    nix_checks: &[String],
    plans: &BTreeMap<String, CheckPlan>,
    backend: &dyn NixBackend,
    operational_error: &mut Option<String>,
) -> BTreeSet<String> {
    let output_paths = nix_checks
        .iter()
        .filter_map(|id| plans.get(id))
        .flat_map(|plan| plan.outputs.iter().cloned())
        .collect::<Vec<_>>();
    if output_paths.is_empty() {
        return BTreeSet::new();
    }
    match backend.validate_outputs(root, &output_paths) {
        Ok(valid) => valid,
        Err(message) => {
            operational_error.get_or_insert(message);
            BTreeSet::new()
        }
    }
}

fn apply_initial_states(
    root: &Path,
    states: BTreeMap<String, InitialState>,
    outcomes: &mut BTreeMap<String, CheckRecord>,
    failures: &mut Vec<FailureDraft>,
    health_report: &mut Option<Value>,
) {
    for (id, state) in states {
        match state {
            InitialState::StaticPassed => set_outcome(outcomes, &id, Outcome::Passed),
            InitialState::StaticFailed(diagnostics) => {
                set_outcome(outcomes, &id, Outcome::Failed);
                failures.extend(diagnostic_failures(root, &id, diagnostics, None));
            }
            InitialState::HealthPassed(health) => {
                set_outcome(outcomes, &id, Outcome::Passed);
                *health_report = Some(health.report);
            }
            InitialState::HealthFailed(health) => {
                set_outcome(outcomes, &id, Outcome::Failed);
                *health_report = Some(health.report);
                failures.extend(health.failures);
            }
            InitialState::HealthDiagnosticFailed(diagnostics) => {
                set_outcome(outcomes, &id, Outcome::Failed);
                failures.extend(diagnostic_failures(root, &id, diagnostics, None));
            }
            InitialState::HealthCanceled => set_outcome(outcomes, &id, Outcome::Canceled),
            InitialState::ClippyCanceled => set_outcome(outcomes, &id, Outcome::Canceled),
            InitialState::Canceled => set_outcome(outcomes, &id, Outcome::Canceled),
            InitialState::NotRun => set_outcome(outcomes, &id, Outcome::NotRun),
        }
    }
}

fn set_outcome(outcomes: &mut BTreeMap<String, CheckRecord>, id: &str, outcome: Outcome) {
    outcomes
        .get_mut(id)
        .expect("selected check outcome")
        .outcome = outcome;
}

struct NixOutcomeInputs<'a> {
    plans: &'a BTreeMap<String, CheckPlan>,
    dependencies: &'a BTreeMap<String, Vec<String>>,
    batch: &'a BatchRealization,
    batch_log: &'a Option<String>,
    batch_canceled: bool,
    valid: &'a BTreeSet<String>,
}

fn apply_nix_outcomes(
    nix_checks: &[String],
    inputs: &NixOutcomeInputs<'_>,
    outcomes: &mut BTreeMap<String, CheckRecord>,
    failures: &mut Vec<FailureDraft>,
) {
    for id in nix_checks {
        let Some(plan) = inputs.plans.get(id) else {
            continue;
        };
        apply_nix_plan(id, plan, inputs, outcomes, failures);
    }
}

fn apply_nix_plan(
    id: &str,
    plan: &CheckPlan,
    inputs: &NixOutcomeInputs<'_>,
    outcomes: &mut BTreeMap<String, CheckRecord>,
    failures: &mut Vec<FailureDraft>,
) {
    let outcome = outcomes.get_mut(id).expect("selected Nix check");
    if let Some(log) = inputs.batch_log {
        outcome.logs.push(log.clone());
    }
    if let Some(error) = &plan.eval_error {
        outcome.outcome = Outcome::Failed;
        failures.push(nix_plan_failure(
            id,
            error,
            inputs.batch_log.clone(),
            failures.len(),
        ));
        return;
    }
    let Some(derivation) = plan.derivation.as_ref() else {
        outcome.outcome = Outcome::NotRun;
        return;
    };
    let all_valid = !plan.outputs.is_empty()
        && plan
            .outputs
            .iter()
            .all(|output| inputs.valid.contains(output));
    let own_block = inputs.batch.failures.get(derivation);
    let dependency_cause = own_block
        .is_none_or(|block| block.log_hint.is_none())
        .then(|| causal_failure(derivation, &inputs.batch.failures, inputs.dependencies))
        .flatten();
    if let Some(cause) = dependency_cause {
        outcome.outcome = Outcome::Failed;
        failures.push(failure_from_block(
            id,
            &cause,
            inputs.batch_log.clone(),
            cause.derivation.clone(),
            failures.len(),
        ));
        return;
    }
    if let Some(block) = own_block {
        outcome.outcome = Outcome::Failed;
        failures.push(failure_from_block(
            id,
            block,
            inputs.batch_log.clone(),
            derivation.clone(),
            failures.len(),
        ));
        return;
    }
    outcome.outcome = if all_valid {
        Outcome::Passed
    } else if inputs.batch_canceled {
        Outcome::Canceled
    } else {
        Outcome::NotRun
    };
}

fn nix_plan_failure(
    id: &str,
    error: &super::catalog::PlanError,
    batch_log: Option<String>,
    occurrence: usize,
) -> FailureDraft {
    FailureDraft {
        check: id.to_owned(),
        code: error.code.clone(),
        subject: None,
        location: None,
        message: error.message.clone(),
        notes: Vec::new(),
        log: batch_log,
        nix_log: None,
        focus_tail: false,
        occurrence,
    }
}

fn run_clippy_suppression_check(root: &Path, allow: &[String]) -> Result<(), Vec<Diagnostic>> {
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
        .map_err(|error| {
            vec![Diagnostic::new(
                "ClippySourceError",
                format!("Unable to discover Cargo workspace targets: {error}"),
            )]
        })?;
    if !output.status.success() {
        return Err(vec![Diagnostic::new(
            "ClippySourceError",
            format!(
                "Cargo target discovery failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        )]);
    }
    let metadata: Value = serde_json::from_slice(&output.stdout).map_err(|error| {
        vec![Diagnostic::new(
            "ClippySourceError",
            format!("Cargo returned invalid target metadata: {error}"),
        )]
    })?;
    let workspace_members = metadata
        .get("workspace_members")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            vec![Diagnostic::new(
                "ClippySourceError",
                "Cargo metadata omitted workspace members",
            )]
        })?
        .iter()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            vec![Diagnostic::new(
                "ClippySourceError",
                "Cargo metadata omitted packages",
            )]
        })?;
    let target_kinds = [
        "lib",
        "bin",
        "example",
        "test",
        "bench",
        "custom-build",
        "proc-macro",
    ];
    let mut source_paths = Vec::new();
    for package in packages {
        if !package
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| workspace_members.contains(id))
        {
            continue;
        }
        let Some(targets) = package.get("targets").and_then(Value::as_array) else {
            continue;
        };
        for target in targets {
            let included = target
                .get("kind")
                .and_then(Value::as_array)
                .is_some_and(|kinds| {
                    kinds
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|kind| target_kinds.contains(&kind))
                });
            if included && let Some(path) = target.get("src_path").and_then(Value::as_str) {
                source_paths.push(PathBuf::from(path));
            }
        }
    }
    bloomery_check::find_clippy_suppressions(root, &source_paths, allow, &AnalysisSession::new())
}

fn failure_from_block(
    check: &str,
    block: &super::catalog::FailureBlock,
    log: Option<String>,
    nix_log: String,
    occurrence: usize,
) -> FailureDraft {
    FailureDraft {
        check: check.to_owned(),
        code: block.code.clone(),
        subject: None,
        location: None,
        message: if block.message.is_empty() {
            "Nix check failed".to_owned()
        } else {
            block.message.clone()
        },
        notes: block.excerpt.clone(),
        log,
        nix_log: block.log_hint.clone().or(Some(nix_log)),
        focus_tail: true,
        occurrence,
    }
}

/// Walk a derivation's input closure for the first failed dependency so a
/// dependency-only failure retains a useful derivation log. The starting
/// derivation is excluded so a check's own failure is not mistaken for a cause.
fn causal_failure(
    derivation: &str,
    failures: &BTreeMap<String, super::catalog::FailureBlock>,
    dependencies: &BTreeMap<String, Vec<String>>,
) -> Option<super::catalog::FailureBlock> {
    let mut queue = VecDeque::new();
    if let Some(inputs) = dependencies.get(derivation) {
        queue.extend(inputs.iter().cloned());
    }
    let mut seen = BTreeSet::new();
    while let Some(current) = queue.pop_front() {
        if !seen.insert(current.clone()) {
            continue;
        }
        if let Some(block) = failures.get(&current) {
            return Some(block.clone());
        }
        if let Some(inputs) = dependencies.get(&current) {
            for input in inputs {
                queue.push_back(input.clone());
            }
        }
    }
    None
}

fn diagnostic_failures(
    root: &Path,
    check: &str,
    diagnostics: Vec<Diagnostic>,
    log: Option<String>,
) -> Vec<FailureDraft> {
    let mut diagnostics = diagnostics;
    sort_model_diagnostics(&mut diagnostics);
    diagnostics
        .into_iter()
        .enumerate()
        .map(|(occurrence, diagnostic)| {
            let location = diagnostic
                .location
                .as_ref()
                .map(|location| FailureLocation {
                    path: display_path(root, &location.path),
                    line: location.line,
                    column: location.column,
                });
            let subject = diagnostic_subject(&diagnostic);
            FailureDraft {
                check: check.to_owned(),
                code: diagnostic.code,
                subject,
                location,
                message: diagnostic.message,
                notes: diagnostic.notes,
                log: log.clone(),
                nix_log: None,
                focus_tail: false,
                occurrence,
            }
        })
        .collect()
}

fn diagnostic_subject(diagnostic: &Diagnostic) -> Option<String> {
    let quoted = diagnostic.message.split('\'').nth(1)?;
    (!quoted.is_empty()).then(|| quoted.to_owned())
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

pub(super) fn assign_failure_ids(mut failures: Vec<FailureDraft>) -> Vec<FailureRecord> {
    failures.sort_by(|left, right| {
        left.check
            .cmp(&right.check)
            .then(left.code.cmp(&right.code))
            .then(left.subject.cmp(&right.subject))
            .then_with(|| {
                let left_location = left
                    .location
                    .as_ref()
                    .map(|location| (&location.path, location.line, location.column));
                let right_location = right
                    .location
                    .as_ref()
                    .map(|location| (&location.path, location.line, location.column));
                left_location.cmp(&right_location)
            })
            .then(left.message.cmp(&right.message))
            .then(left.notes.cmp(&right.notes))
            .then(left.occurrence.cmp(&right.occurrence))
    });
    failures
        .into_iter()
        .enumerate()
        .map(|(index, failure)| FailureRecord {
            id: format!("f{}", index + 1),
            check: failure.check,
            code: failure.code,
            subject: failure.subject,
            location: failure.location,
            message: failure.message,
            notes: failure.notes,
            log: failure.log,
            nix_log: failure.nix_log,
            focus_tail: failure.focus_tail,
        })
        .collect()
}
