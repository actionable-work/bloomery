use super::catalog::{BatchRealization, CheckPlan, NixBackend, NixTaskResult};
use super::command::{CheckArgs, CheckContext, CheckError, plan_for_selection, report_error};
use super::interrupt::{CancellationToken, InterruptFlag};
use super::model::{
    CheckRecord, FailureLocation, FailureRecord, Outcome, RunRecord, RunSelection, RunStatus,
    select_ids,
};
use super::output;
use super::progress::{ProgressEvent, ProgressReducer, ProgressReporter, ProgressSink};
use super::scheduler::{ScheduleReport, ScheduledState, WorkerTracker, run_with_followups};
use super::store;
use bloomery_check::AnalysisSession;
use bloomery_model::{
    Diagnostic, health::HealthConfig, sort_diagnostics as sort_model_diagnostics,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::Write;
use std::path::Path;
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
    run_prepared_checks(args, context, prepared, stdout, stderr)
}

struct PreparedCheckRun {
    planned: super::command::PlannedCatalog,
    selected: BTreeSet<String>,
    health_sources: Option<Result<Vec<bloomery_check::HealthSourceCrate>, String>>,
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
    let planned = plan_for_selection(
        context.root,
        &args.selectors,
        &args.systems,
        context.backend,
    )?;
    let (selected, selection) = prepare_selection(args, &planned)?;
    let health_sources = prepare_health_sources(context.root, context.backend, &selected)?;
    let store =
        store::RunStore::open(context.root, context.cache_base).map_err(CheckError::operational)?;
    let allocation = store.allocate().map_err(CheckError::operational)?;
    Ok(PreparedCheckRun {
        planned,
        selected,
        health_sources,
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

fn prepare_health_sources(
    root: &Path,
    backend: &dyn NixBackend,
    selected: &BTreeSet<String>,
) -> Result<Option<Result<Vec<bloomery_check::HealthSourceCrate>, String>>, CheckError> {
    if !selected.contains("static:code-health") {
        return Ok(None);
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
        validate_health_source_overlap(root, &config, &raw_json, sources)
            .map_err(CheckError::operational)?;
    }
    Ok(Some(projection))
}

struct CheckExecution {
    format: FormatGate,
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
        ProgressReducer::new(prepared.selected.len() + 1, has_nix_checks),
        stderr,
        eligible,
        context.terminal.width(),
        context.terminal.progress_styled(),
    );
    progress.start();
    let format = run_format_gate(
        context.root,
        context.backend,
        &prepared.allocation,
        &context.interrupt,
        &mut progress,
    );
    let source = store::source_metadata(prepared.store.root());
    let results = execute_after_formatter(args, context, prepared, &format, &mut progress);
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
    format: &FormatGate,
    progress: &mut ProgressReporter<impl Write>,
) -> Result<ExecutionResults, String> {
    if format.outcome == Outcome::Passed {
        return execute_selected(
            context.root,
            &prepared.selected,
            &prepared.allocation,
            context.backend,
            &prepared.planned.plans,
            &prepared.planned.dependencies,
            prepared.health_sources.clone(),
            context.interrupt.clone(),
            prepared.jobs,
            args.fail_fast,
            progress,
        );
    }
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
            let outcomes = operational_outcomes(&prepared.selected, format.outcome, &format.log);
            let record = RunRecord {
                id: prepared.allocation.id.clone(),
                root: prepared.store.root().to_string_lossy().into_owned(),
                started_at: prepared.started_at,
                completed_at,
                source_revision: source.revision,
                source_dirty: source.dirty,
                status: RunStatus::Error,
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
    mut format: FormatGate,
    source: store::SourceMetadata,
    mut results: ExecutionResults,
    completed_at: u64,
    final_metrics: Option<String>,
) -> (RunRecord, Option<String>, Option<String>) {
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
    let mut operational_error = results.operational_error.take();
    let health_report_available = results.health_report.is_some();
    retain_health_report(
        &prepared.allocation,
        &mut results.health_report,
        &mut operational_error,
    );
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
        selection: prepared.selection.clone(),
        outcomes: results.outcomes.into_values().collect(),
        failures,
        notices,
    };
    (record, operational_error, final_metrics)
}

fn operational_outcomes(
    selected: &BTreeSet<String>,
    format_outcome: Outcome,
    format_log: &Option<String>,
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
    outcomes.push(CheckRecord {
        id: FORMAT_WORKSPACE_ID.to_owned(),
        outcome: format_outcome,
        blocked_by: None,
        logs: format_log.clone().into_iter().collect(),
    });
    outcomes.sort_by(|left, right| left.id.cmp(&right.id));
    outcomes
}

fn retain_health_report(
    allocation: &store::RunAllocation,
    report: &mut Option<Value>,
    operational_error: &mut Option<String>,
) {
    if let Some(report) = report.take()
        && let Err(error) = allocation.retain_health_report(&report)
    {
        operational_error.get_or_insert(error);
    }
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

pub(super) struct FailureDraft {
    pub(super) check: String,
    pub(super) code: String,
    pub(super) subject: Option<String>,
    pub(super) location: Option<FailureLocation>,
    pub(super) message: String,
    pub(super) notes: Vec<String>,
    pub(super) log: Option<String>,
    pub(super) nix_log: Option<String>,
    pub(super) focus_tail: bool,
    pub(super) occurrence: usize,
}

type InitialWork<'a> = (
    String,
    Box<dyn FnOnce(CancellationToken) -> InitialTask + Send + 'a>,
);

struct InitialTask {
    result: InitialTaskResult,
}

#[allow(clippy::large_enum_variant)]
enum InitialTaskResult {
    Structure(Result<Option<bloomery_model::Context>, Vec<Diagnostic>>),
    Traceability(Result<(), Vec<Diagnostic>>),
    Health(Result<HealthOutcome, Vec<Diagnostic>>),
    HealthCanceled,
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
    Canceled,
    NotRun,
}

struct HealthOutcome {
    report: Value,
    failures: Vec<FailureDraft>,
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
    health_sources: Option<Result<Vec<bloomery_check::HealthSourceCrate>, String>>,
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
        health_sources,
        fail_fast,
        sink,
    } = inputs;
    let mut tasks = Vec::new();
    if selected.contains("static:structure") {
        let root = root.to_path_buf();
        tasks.push((
            "static:structure".to_owned(),
            Box::new(move |_cancellation: CancellationToken| InitialTask {
                result: InitialTaskResult::Structure(bloomery_workspace::load(&root).map(Some)),
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
                    InitialTaskResult::Health(collect_health_report(&root, &sources, &analysis))
                },
            }) as _,
        ));
    }
    if !batch_plans.is_empty() {
        tasks.push(batch_task(
            root,
            allocation,
            backend,
            batch_plans,
            fail_fast,
            sink,
        ));
    }
    tasks
}

fn batch_task<'a>(
    root: &Path,
    allocation: &store::RunAllocation,
    backend: &'a dyn NixBackend,
    batch_plans: Vec<CheckPlan>,
    fail_fast: bool,
    sink: &ProgressSink,
) -> InitialWork<'a> {
    let root = root.to_path_buf();
    let allocation = allocation.clone();
    let sink = sink.clone();
    (
        "nix:batch".to_owned(),
        Box::new(move |cancellation: CancellationToken| {
            let log_name = "nix-batch.log";
            let result = match allocation.prepare_log(log_name) {
                Ok(log_path) => backend.realize_batch(
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

#[allow(clippy::too_many_arguments)]
fn execute_selected<W: Write>(
    root: &Path,
    selected: &BTreeSet<String>,
    allocation: &store::RunAllocation,
    backend: &dyn NixBackend,
    plans: &BTreeMap<String, CheckPlan>,
    dependencies: &BTreeMap<String, Vec<String>>,
    health_sources: Option<Result<Vec<bloomery_check::HealthSourceCrate>, String>>,
    interrupt: InterruptFlag,
    jobs: usize,
    fail_fast: bool,
    progress: &mut ProgressReporter<W>,
) -> Result<ExecutionResults, String> {
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

    let (event_sender, event_receiver) = mpsc::channel();
    let sink = ProgressSink::new(event_sender, progress.is_enabled());
    let initial_tasks = build_initial_tasks(InitialTaskInputs {
        root,
        selected,
        allocation,
        backend,
        batch_plans,
        health_sources: health_sources.clone(),
        fail_fast,
        sink: &sink,
    });
    let initial_report = execute_initial_schedule(
        initial_tasks,
        InitialScheduleInputs {
            root,
            selected,
            health_sources: health_sources.as_ref(),
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
    apply_initial_states(
        root,
        reduction.states,
        &mut outcomes,
        &mut failures,
        &mut health_report,
    );
    let nix_inputs = NixOutcomeInputs {
        plans,
        dependencies,
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
        InitialTaskResult::SharedAnalysis { .. } => None,
        InitialTaskResult::NixBatch { .. } => None,
    }
}

struct InitialScheduleInputs<'a, W: Write> {
    root: &'a Path,
    selected: &'a BTreeSet<String>,
    health_sources: Option<&'a Result<Vec<bloomery_check::HealthSourceCrate>, String>>,
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
        jobs,
        interrupt,
        fail_fast,
        sink,
        event_receiver,
        progress,
    } = inputs;
    run_with_followups(
        tasks,
        jobs,
        interrupt,
        |task| initial_task_stops_admission(task, fail_fast),
        |task| followup_work(root, selected, health_sources, task),
        |id, task| publish_completion_events(id, task, selected, sink),
        |queued| publish_not_run_events(queued, sink),
        |tracker| drain_progress_events(tracker, event_receiver, progress),
    )
}

fn followup_work<'a>(
    root: &'a Path,
    selected: &BTreeSet<String>,
    health_sources: Option<&Result<Vec<bloomery_check::HealthSourceCrate>, String>>,
    task: &mut InitialTask,
) -> Vec<InitialWork<'a>> {
    if let InitialTaskResult::Structure(Ok(context)) = &mut task.result
        && let Some(context) = context.take()
        && selected.contains("static:traceability")
    {
        return vec![shared_analysis_task(
            root,
            context,
            selected.contains("static:code-health"),
            health_sources,
        )];
    }
    if matches!(&task.result, InitialTaskResult::Structure(Err(_)))
        && selected.contains("static:code-health")
        && selected.contains("static:traceability")
    {
        return vec![standalone_health_task(root, health_sources)];
    }
    Vec::new()
}

fn shared_analysis_task<'a>(
    root: &'a Path,
    context: bloomery_model::Context,
    health_selected: bool,
    health_sources: Option<&Result<Vec<bloomery_check::HealthSourceCrate>, String>>,
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
                    collect_health_report(root, &sources, &analysis)
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
                InitialTaskResult::Health(collect_health_report(&root, &sources, &analysis))
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
    }
    while let Ok(event) = event_receiver.try_recv() {
        progress.apply(event);
    }
}

fn initial_task_stops_admission(task: &InitialTask, fail_fast: bool) -> bool {
    match &task.result {
        InitialTaskResult::Structure(Err(_))
        | InitialTaskResult::Traceability(Err(_))
        | InitialTaskResult::Health(Err(_)) => fail_fast,
        InitialTaskResult::Health(Ok(health)) => fail_fast && !health.failures.is_empty(),
        InitialTaskResult::NixBatch { result, .. } => {
            result.operational_error.is_some() || (fail_fast && !result.failures.is_empty())
        }
        InitialTaskResult::Structure(Ok(_))
        | InitialTaskResult::Traceability(Ok(()))
        | InitialTaskResult::HealthCanceled => false,
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
            InitialTaskResult::HealthCanceled => Some(InitialState::HealthCanceled),
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

fn collect_health_report(
    root: &Path,
    source_projection: &Result<Vec<bloomery_check::HealthSourceCrate>, String>,
    session: &AnalysisSession,
) -> Result<HealthOutcome, Vec<Diagnostic>> {
    let loaded =
        bloomery_model::config::load_with_raw(root).map_err(|diagnostic| vec![diagnostic])?;
    let config =
        HealthConfig::from_raw(loaded.raw.as_ref()).map_err(|diagnostic| vec![diagnostic])?;
    let sources = source_projection
        .clone()
        .map_err(|error| vec![Diagnostic::new("HealthDiscoveryError", error)])?;
    let mut crates =
        bloomery_check::analyze_health(root, &sources, config.duplicate_min_tokens, session)?;
    if let Err(message) = validate_health_metrics_overlap(root, &config, &crates) {
        return Err(vec![Diagnostic::new("ConfigurationError", message)]);
    }
    crates.sort_by(|left, right| left.manifest_path.cmp(&right.manifest_path));
    let scores = crates
        .iter()
        .map(|crate_metrics| bloomery_check::health::score_crate(crate_metrics, &config))
        .collect::<Vec<_>>();
    let workspace = bloomery_check::health::workspace_score(&crates, &scores);
    let mut failures = bloomery_check::health::policy_gates(&crates, &scores, workspace, &config)
        .into_iter()
        .map(|gate| {
            let location = gate.path.map(|path| FailureLocation {
                path,
                line: gate.line,
                column: None,
            });
            FailureDraft {
                check: "static:code-health".to_owned(),
                code: gate.code,
                subject: gate.subject,
                location,
                message: gate.message,
                notes: Vec::new(),
                log: None,
                nix_log: None,
                focus_tail: false,
                occurrence: 0,
            }
        })
        .collect::<Vec<_>>();
    let raw_json = loaded
        .raw
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|error| vec![Diagnostic::new("HealthInputError", error.to_string())])?;
    let policy = health_policy_value(&config, &raw_json);
    let input_digest = health_input_digest(root, &crates, &sources, &policy, session)
        .map_err(|error| vec![Diagnostic::new("HealthInputError", error)])?;
    let snapshot = health_snapshot(&crates, &scores, workspace, policy, input_digest);
    let mut report = snapshot.clone();
    failures.extend(compare_health_baseline(
        root,
        &config,
        &crates,
        &scores,
        workspace,
        &snapshot,
        &mut report,
    ));
    failures.sort_by(|left, right| {
        (
            &left.code,
            &left.subject,
            left.location
                .as_ref()
                .map(|location| (&location.path, location.line)),
            &left.message,
        )
            .cmp(&(
                &right.code,
                &right.subject,
                right
                    .location
                    .as_ref()
                    .map(|location| (&location.path, location.line)),
                &right.message,
            ))
    });
    for (index, failure) in failures.iter_mut().enumerate() {
        failure.occurrence = index;
    }
    Ok(HealthOutcome { report, failures })
}

fn compare_health_baseline(
    root: &Path,
    config: &HealthConfig,
    crates: &[bloomery_check::HealthCrateMetrics],
    scores: &[bloomery_check::health::CrateScore],
    workspace: Option<u16>,
    snapshot: &Value,
    report: &mut Value,
) -> Vec<FailureDraft> {
    let (status, baseline, mut failures) = read_health_baseline(root, config, workspace);
    report["baseline_status"] = json!(status);
    report["baseline_workspace_score_hundredths"] = Value::Null;
    report["workspace_delta_hundredths"] = Value::Null;
    report["added_crates"] = json!([]);
    report["removed_crates"] = json!([]);
    let Some(baseline) = baseline else {
        return failures;
    };
    match validate_health_baseline(&baseline, snapshot) {
        Ok(baseline) => compare_compatible_baseline(
            config,
            crates,
            scores,
            workspace,
            baseline.workspace_score_hundredths,
            &baseline.crates,
            report,
            &mut failures,
        ),
        Err(message) => {
            report["baseline_status"] = json!("incompatible");
            failures.push(health_failure(
                "HealthBaselineIncompatible",
                message,
                Some(config.baseline.clone()),
                None,
            ));
        }
    }
    failures
}

fn read_health_baseline(
    root: &Path,
    config: &HealthConfig,
    workspace: Option<u16>,
) -> (&'static str, Option<Value>, Vec<FailureDraft>) {
    let result = std::fs::read(root.join(&config.baseline));
    match result {
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(value) => ("loaded", Some(value), Vec::new()),
            Err(error) => (
                "malformed",
                None,
                vec![health_failure(
                    "HealthBaselineInvalid",
                    format!("code health baseline is malformed: {error}"),
                    Some(config.baseline.clone()),
                    None,
                )],
            ),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let failures = workspace
                .is_some()
                .then(|| {
                    health_failure(
                        "HealthBaselineMissing",
                        format!("code health baseline '{}' does not exist", config.baseline),
                        Some(config.baseline.clone()),
                        None,
                    )
                })
                .into_iter()
                .collect();
            ("missing", None, failures)
        }
        Err(error) => (
            "unreadable",
            None,
            vec![health_failure(
                "HealthBaselineInvalid",
                format!("unable to read code health baseline: {error}"),
                Some(config.baseline.clone()),
                None,
            )],
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn compare_compatible_baseline(
    config: &HealthConfig,
    crates: &[bloomery_check::HealthCrateMetrics],
    scores: &[bloomery_check::health::CrateScore],
    workspace: Option<u16>,
    baseline_workspace: Option<u16>,
    baseline_crates: &BTreeMap<String, Option<u16>>,
    report: &mut Value,
    failures: &mut Vec<FailureDraft>,
) {
    report["baseline_status"] = json!("compatible");
    report["baseline_workspace_score_hundredths"] =
        baseline_workspace.map_or(Value::Null, |score| json!(score));
    compare_workspace_score(config, workspace, baseline_workspace, report, failures);
    compare_crate_membership(crates, baseline_crates, report);
    report["crate_comparisons"] = json!(compare_crate_scores(
        config,
        crates,
        scores,
        baseline_crates,
        failures,
    ));
}

fn compare_workspace_score(
    config: &HealthConfig,
    current: Option<u16>,
    previous: Option<u16>,
    report: &mut Value,
    failures: &mut Vec<FailureDraft>,
) {
    if let (Some(current), Some(previous)) = (current, previous) {
        let delta = i32::from(current) - i32::from(previous);
        report["workspace_delta_hundredths"] = json!(delta);
        if delta < -i32::from(config.max_regression) {
            failures.push(health_failure(
                "HealthWorkspaceRegression",
                regression_message("workspace", delta, config.max_regression),
                None,
                None,
            ));
        }
    }
}

fn compare_crate_membership(
    crates: &[bloomery_check::HealthCrateMetrics],
    baseline_crates: &BTreeMap<String, Option<u16>>,
    report: &mut Value,
) {
    let current_ids = crates
        .iter()
        .map(|crate_metrics| crate_metrics.manifest_path.as_str())
        .collect::<BTreeSet<_>>();
    let previous_ids = baseline_crates
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    report["added_crates"] = json!(current_ids.difference(&previous_ids).collect::<Vec<_>>());
    report["removed_crates"] = json!(previous_ids.difference(&current_ids).collect::<Vec<_>>());
}

fn compare_crate_scores(
    config: &HealthConfig,
    crates: &[bloomery_check::HealthCrateMetrics],
    scores: &[bloomery_check::health::CrateScore],
    baseline_crates: &BTreeMap<String, Option<u16>>,
    failures: &mut Vec<FailureDraft>,
) -> Vec<Value> {
    crates
        .iter()
        .zip(scores)
        .map(|(crate_metrics, score)| {
            let previous = baseline_crates
                .get(&crate_metrics.manifest_path)
                .copied()
                .flatten();
            let delta = score
                .score_hundredths
                .zip(previous)
                .map(|(now, before)| i32::from(now) - i32::from(before));
            if let Some(delta) = delta
                && delta < -i32::from(config.max_regression)
            {
                failures.push(health_failure(
                    "HealthCrateRegression",
                    regression_message("crate", delta, config.max_regression),
                    Some(crate_metrics.manifest_path.clone()),
                    None,
                ));
            }
            json!({
                "manifest_path": crate_metrics.manifest_path,
                "baseline_score_hundredths": previous,
                "score_hundredths": score.score_hundredths,
                "delta_hundredths": delta,
            })
        })
        .collect()
}

fn regression_message(scope: &str, delta: i32, allowance: u16) -> String {
    format!(
        "{scope} score regressed by {:.2} points, exceeding the {:.2} point allowance",
        -delta as f64 / 100.0,
        allowance as f64 / 100.0,
    )
}

fn health_failure(
    code: &str,
    message: String,
    subject: Option<String>,
    location: Option<FailureLocation>,
) -> FailureDraft {
    FailureDraft {
        check: "static:code-health".to_owned(),
        code: code.to_owned(),
        subject,
        location,
        message,
        notes: Vec::new(),
        log: None,
        nix_log: None,
        focus_tail: false,
        occurrence: 0,
    }
}

fn validate_health_source_overlap(
    root: &Path,
    config: &HealthConfig,
    raw: &Option<Value>,
    sources: &[bloomery_check::HealthSourceCrate],
) -> Result<(), String> {
    let root = std::fs::canonicalize(root)
        .map_err(|error| format!("unable to resolve repository root: {error}"))?;
    let baseline = Path::new(&config.baseline);
    let candidate = root.join(baseline);
    let selected_manifest = raw
        .as_ref()
        .and_then(|value| value.get("build"))
        .and_then(Value::as_object)
        .and_then(|build| build.get("cargoToml"))
        .and_then(Value::as_str)
        .unwrap_or("Cargo.toml");
    let mut protected = BTreeSet::from([
        root.join(".bloomery/config.toml"),
        root.join(".bloomery/specs"),
        root.join("Cargo.lock"),
        root.join("flake.lock"),
        root.join(selected_manifest),
    ]);
    for crate_source in sources {
        protected.insert(root.join(&crate_source.manifest_path));
        let crate_directory = root
            .join(&crate_source.manifest_path)
            .parent()
            .unwrap_or(&root)
            .to_path_buf();
        for target in &crate_source.targets {
            protected.insert(crate_directory.join(&target.path));
        }
    }
    if protected
        .iter()
        .any(|path| candidate.starts_with(path) || path.starts_with(&candidate))
    {
        return Err("checks.codeHealth.baseline overlaps a manifest, source entrypoint, configuration, lockfile, or specification".to_owned());
    }
    Ok(())
}

fn validate_health_metrics_overlap(
    root: &Path,
    config: &HealthConfig,
    crates: &[bloomery_check::HealthCrateMetrics],
) -> Result<(), String> {
    let root = std::fs::canonicalize(root)
        .map_err(|error| format!("unable to resolve repository root: {error}"))?;
    let candidate = root.join(&config.baseline);
    if crates.iter().any(|crate_metrics| {
        crate_metrics.sources.iter().any(|source| {
            candidate == root.join(source) || root.join(source).starts_with(&candidate)
        })
    }) {
        return Err("checks.codeHealth.baseline overlaps a resolved Rust source file".to_owned());
    }
    Ok(())
}

pub(super) fn health_policy_value(config: &HealthConfig, raw: &Option<Value>) -> Value {
    let build = raw
        .as_ref()
        .and_then(|value| value.get("build"))
        .and_then(Value::as_object);
    let selected_manifest = build
        .and_then(|build| build.get("cargoToml"))
        .and_then(Value::as_str)
        .unwrap_or("Cargo.toml");
    let members = build
        .and_then(|build| build.get("members"))
        .and_then(Value::as_array)
        .map(|members| {
            let mut members = members.iter().filter_map(Value::as_str).collect::<Vec<_>>();
            members.sort();
            json!(members)
        })
        .unwrap_or(Value::Null);
    json!({
        "scoring": {
            "cyclomaticLimit": config.cyclomatic_limit,
            "functionTokensLimit": config.function_tokens_limit,
            "nestingLimit": config.nesting_limit,
            "duplicationFreeRatio": config.duplication_free_ratio,
            "duplicateMinTokens": config.duplicate_min_tokens,
        },
        "selection": {
            "manifest": selected_manifest,
            "members": members,
        },
        "effective": {
            "enable": config.enabled,
            "baseline": config.baseline,
            "minCrateScore": config.min_crate_score,
            "minWorkspaceScore": config.min_workspace_score,
            "maxRegression": config.max_regression,
            "maxCyclomatic": config.max_cyclomatic,
            "maxFunctionTokens": config.max_function_tokens,
            "maxNesting": config.max_nesting,
        },
    })
}

fn health_input_digest(
    root: &Path,
    crates: &[bloomery_check::HealthCrateMetrics],
    sources: &[bloomery_check::HealthSourceCrate],
    policy: &Value,
    session: &AnalysisSession,
) -> Result<String, String> {
    let mut hasher = Sha256::new();
    let mut inputs = BTreeSet::new();
    let selected_manifest = policy
        .pointer("/selection/manifest")
        .and_then(Value::as_str)
        .unwrap_or("Cargo.toml");
    inputs.insert(selected_manifest.to_owned());
    inputs.insert(".bloomery/config.toml".to_owned());
    for crate_source in sources {
        inputs.insert(crate_source.manifest_path.clone());
    }
    let source_paths = crates
        .iter()
        .flat_map(|crate_metrics| crate_metrics.sources.iter().cloned())
        .collect::<BTreeSet<_>>();
    for path in inputs {
        let full = root.join(&path);
        let contents = std::fs::read(&full)
            .map_err(|error| format!("unable to read health discovery input {}: {error}", path))?;
        digest_field(&mut hasher, path.as_bytes());
        digest_field(&mut hasher, &contents);
    }
    let discovery = health_discovery_value(sources);
    let discovery_bytes = serde_json::to_vec(&discovery)
        .map_err(|error| format!("unable to serialize health source discovery: {error}"))?;
    digest_field(&mut hasher, &discovery_bytes);
    for path in source_paths {
        let parsed = session
            .rust_file(&root.join(&path))
            .map_err(|error| format!("unable to retrieve cached Rust source {path}: {error}"))?;
        digest_field(&mut hasher, path.as_bytes());
        digest_field(&mut hasher, parsed.source.as_bytes());
    }
    let policy_bytes = serde_json::to_vec(policy)
        .map_err(|error| format!("unable to serialize effective health policy: {error}"))?;
    digest_field(&mut hasher, &policy_bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

pub(super) fn health_discovery_value(sources: &[bloomery_check::HealthSourceCrate]) -> Value {
    let mut crates = sources
        .iter()
        .map(|crate_source| {
            let mut targets = crate_source
                .targets
                .iter()
                .map(|target| json!({"kind": target.kind, "path": target.path}))
                .collect::<Vec<_>>();
            targets.sort_by(|left, right| {
                left.get("path")
                    .and_then(Value::as_str)
                    .cmp(&right.get("path").and_then(Value::as_str))
                    .then(
                        left.get("kind")
                            .and_then(Value::as_str)
                            .cmp(&right.get("kind").and_then(Value::as_str)),
                    )
            });
            json!({
                "name": crate_source.name,
                "manifest_path": crate_source.manifest_path,
                "targets": targets,
            })
        })
        .collect::<Vec<_>>();
    crates.sort_by(|left, right| {
        left.get("manifest_path")
            .and_then(Value::as_str)
            .cmp(&right.get("manifest_path").and_then(Value::as_str))
    });
    json!(crates)
}

fn digest_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn health_snapshot(
    crates: &[bloomery_check::HealthCrateMetrics],
    scores: &[bloomery_check::health::CrateScore],
    workspace: Option<u16>,
    policy: Value,
    input_digest: String,
) -> Value {
    let crate_records = crates
        .iter()
        .zip(scores)
        .map(|(crate_metrics, score)| {
            let callables = crate_metrics
                .callables
                .iter()
                .map(|callable| {
                    json!({
                        "identity": callable.identity,
                        "path": callable.path,
                        "start": {"line": callable.start_line, "column": callable.start_column},
                        "end": {"line": callable.end_line, "column": callable.end_column},
                        "cyclomatic": callable.cyclomatic,
                        "nesting": callable.nesting,
                        "body_tokens": callable.body_tokens.len(),
                    })
                })
                .collect::<Vec<_>>();
            let ranges = crate_metrics
                .callables
                .iter()
                .filter(|callable| callable.duplicate_token_count > 0)
                .map(|callable| {
                    json!({
                        "path": callable.path,
                        "start_line": callable.start_line,
                        "end_line": callable.end_line,
                        "covered_tokens": callable.duplicate_token_count,
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "manifest_path": crate_metrics.manifest_path,
                "name": crate_metrics.name,
                "sources": crate_metrics.sources,
                "opaque_sources": crate_metrics.opaque_sources.iter().map(|item| json!({
                    "path": item.path,
                    "line": item.line,
                    "description": item.description,
                })).collect::<Vec<_>>(),
                "sloc": crate_metrics.sloc,
                "callables": callables,
                "duplication": {
                    "covered_tokens": crate_metrics.duplicate_tokens,
                    "total_tokens": crate_metrics.callable_tokens,
                    "fraction": score.duplication_fraction,
                    "ranges": ranges,
                },
                "penalties": {
                    "complexity": score.penalties.complexity,
                    "size": score.penalties.size,
                    "nesting": score.penalties.nesting,
                    "duplication": score.penalties.duplication,
                },
                "score_hundredths": score.score_hundredths,
                "applicability": if score.score_hundredths.is_some() { "applicable" } else { "not_applicable" },
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": 1,
        "algorithm_version": 1,
        "policy": policy,
        "input_digest": input_digest,
        "crates": crate_records,
        "workspace": {
            "total_sloc": crates.iter().map(|crate_metrics| crate_metrics.sloc).sum::<usize>(),
            "applicable_crates": scores.iter().filter(|score| score.score_hundredths.is_some()).count(),
            "score_hundredths": workspace,
        }
    })
}

#[derive(Debug)]
struct ValidatedHealthBaseline {
    crates: BTreeMap<String, Option<u16>>,
    workspace_score_hundredths: Option<u16>,
}

fn validate_health_baseline(
    baseline: &Value,
    current: &Value,
) -> Result<ValidatedHealthBaseline, String> {
    if baseline.get("schema_version").and_then(Value::as_u64) != Some(3) {
        return Err("baseline has an unsupported schema version".to_owned());
    }
    if baseline.get("algorithm_version").and_then(Value::as_u64) != Some(1) {
        return Err("baseline has an unsupported health algorithm version".to_owned());
    }
    if baseline.pointer("/policy/scoring") != current.pointer("/policy/scoring")
        || baseline.pointer("/policy/selection") != current.pointer("/policy/selection")
    {
        return Err("baseline scoring limits or source-selection policy do not match".to_owned());
    }
    let digest = baseline
        .get("input_digest")
        .and_then(Value::as_str)
        .unwrap_or("");
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("baseline has an invalid input digest".to_owned());
    }
    let crate_records = baseline
        .get("crates")
        .and_then(Value::as_array)
        .ok_or_else(|| "baseline has no crate array".to_owned())?;
    let mut scores = BTreeMap::new();
    let mut total_sloc = 0_u128;
    let mut weighted_score = 0_u128;
    for crate_record in crate_records {
        let manifest = crate_record
            .get("manifest_path")
            .and_then(Value::as_str)
            .filter(|manifest| !manifest.is_empty())
            .ok_or_else(|| "baseline crate is missing manifest_path".to_owned())?;
        let sloc = crate_record
            .get("sloc")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("baseline crate '{manifest}' has no valid SLOC"))?;
        let raw_score = crate_record
            .get("overall_score")
            .ok_or_else(|| format!("baseline crate '{manifest}' has no overall score"))?;
        let score = parse_baseline_score(raw_score)
            .ok_or_else(|| format!("baseline crate '{manifest}' has an invalid overall score"))?;
        if (sloc == 0) != score.is_none() {
            return Err(format!(
                "baseline crate '{manifest}' has inconsistent score applicability"
            ));
        }
        total_sloc += u128::from(sloc);
        if let Some(score) = score {
            weighted_score += u128::from(score) * u128::from(sloc);
        }
        if scores.insert(manifest.to_owned(), score).is_some() {
            return Err(format!(
                "baseline has duplicate crate identity '{manifest}'"
            ));
        }
    }
    let derived_workspace_score = (weighted_score + total_sloc / 2)
        .checked_div(total_sloc)
        .map(|score| score as u16);
    let workspace = baseline
        .get("workspace")
        .ok_or_else(|| "baseline has no workspace summary".to_owned())?;
    if workspace.get("sloc").and_then(Value::as_u64) != u64::try_from(total_sloc).ok() {
        return Err("baseline workspace SLOC is inconsistent with crate rows".to_owned());
    }
    let workspace_score = parse_baseline_score(
        workspace
            .get("overall_score")
            .ok_or_else(|| "baseline workspace has no overall score".to_owned())?,
    )
    .ok_or_else(|| "baseline workspace score is invalid".to_owned())?;
    if workspace_score != derived_workspace_score {
        return Err("baseline workspace score is inconsistent with crate rows".to_owned());
    }
    Ok(ValidatedHealthBaseline {
        crates: scores,
        workspace_score_hundredths: workspace_score,
    })
}

fn parse_baseline_score(value: &Value) -> Option<Option<u16>> {
    if value.is_null() {
        return Some(None);
    }
    let score = value.as_f64()?;
    let hundredths = (score * 100.0).round();
    if !score.is_finite()
        || !(0.0..=100.0).contains(&score)
        || (score * 100.0 - hundredths).abs() > 1e-7
    {
        return None;
    }
    Some(Some(hundredths as u16))
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

#[cfg(test)]
mod health_baseline_tests {
    use super::validate_health_baseline;
    use serde_json::{Value, json};

    fn valid_snapshot() -> Value {
        json!({
            "schema_version": 3,
            "algorithm_version": 1,
            "input_digest": "a".repeat(64),
            "policy": {
                "scoring": {"cyclomaticLimit": 10},
                "selection": {"include": "production"}
            },
            "crates": [{
                "manifest_path": "crates/demo/Cargo.toml",
                "sloc": 10,
                "overall_score": 100.0
            }],
            "workspace": {
                "sloc": 10,
                "overall_score": 100.0
            }
        })
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-030"))]
    fn baseline_validation_rejects_duplicate_and_inconsistent_records() {
        let valid = valid_snapshot();
        let baseline = validate_health_baseline(&valid, &valid).expect("valid baseline");
        assert_eq!(baseline.crates.len(), 1);
        assert_eq!(baseline.workspace_score_hundredths, Some(10_000));

        let mut duplicate = valid.clone();
        let original = duplicate["crates"][0].clone();
        duplicate["crates"]
            .as_array_mut()
            .expect("crate array")
            .push(original);
        assert!(
            validate_health_baseline(&duplicate, &valid)
                .expect_err("duplicate identity")
                .contains("duplicate crate identity")
        );

        let mut invalid_score = valid.clone();
        invalid_score["crates"][0]["overall_score"] = json!(100.01);
        assert!(
            validate_health_baseline(&invalid_score, &valid)
                .expect_err("out-of-range score")
                .contains("invalid overall score")
        );

        let mut inconsistent_crate = valid.clone();
        inconsistent_crate["crates"][0]["sloc"] = json!(0);
        assert!(
            validate_health_baseline(&inconsistent_crate, &valid)
                .expect_err("inconsistent score applicability")
                .contains("inconsistent score applicability")
        );

        let mut inconsistent_workspace = valid.clone();
        inconsistent_workspace["workspace"]["overall_score"] = json!(99.99);
        assert!(
            validate_health_baseline(&inconsistent_workspace, &valid)
                .expect_err("inconsistent workspace score")
                .contains("workspace score is inconsistent")
        );
    }
}
