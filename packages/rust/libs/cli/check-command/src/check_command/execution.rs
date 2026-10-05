use super::catalog::{BatchRealization, CheckPlan, NixBackend, NixTaskResult};
use super::command::{CheckArgs, CheckContext, CheckError, plan_for_selection, report_error};
use super::interrupt::{CancellationToken, InterruptFlag};
use super::model::{
    CheckRecord, FailureLocation, FailureRecord, Outcome, RunRecord, RunSelection, RunStatus,
    select_ids,
};
use super::output;
use super::progress::{ProgressEvent, ProgressReducer, ProgressReporter, ProgressSink};
use super::scheduler::{ScheduledState, WorkerTracker, run_with_followups};
use super::store;
use bloomery_model::{Diagnostic, sort_diagnostics as sort_model_diagnostics};
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
    let root = context.root;
    let json_mode = context.json_mode;
    let backend = context.backend;
    let interrupt = context.interrupt.clone();
    let cache_base = context.cache_base;
    let planned = match plan_for_selection(root, &args.selectors, &args.systems, backend) {
        Ok(planned) => planned,
        Err(error) => return report_error(json_mode, &error, None, stdout, stderr),
    };
    let direct_selection = match select_ids(&planned.ids, &args.selectors) {
        Ok(ids) => ids,
        Err(message) => {
            return report_error(json_mode, &CheckError::usage(message), None, stdout, stderr);
        }
    };
    if let Err(error) = validate_execution_selection(&direct_selection) {
        return report_error(json_mode, &error, None, stdout, stderr);
    }

    let mut selected = direct_selection.iter().cloned().collect::<BTreeSet<_>>();
    if selected.contains("static:traceability") {
        selected.insert("static:structure".to_owned());
    }
    if let Err(error) = validate_execution_selection(&selected.iter().cloned().collect::<Vec<_>>())
    {
        return report_error(json_mode, &error, None, stdout, stderr);
    }

    let selection = RunSelection {
        selectors: args.selectors.clone(),
        systems: planned.systems.clone(),
        selected_checks: selected.iter().cloned().collect(),
        partial: !args.selectors.is_empty() || !args.systems.is_empty(),
    };
    let store = match store::RunStore::open(root, cache_base) {
        Ok(store) => store,
        Err(message) => {
            return report_error(
                json_mode,
                &CheckError::operational(message),
                None,
                stdout,
                stderr,
            );
        }
    };
    let allocation = match store.allocate() {
        Ok(allocation) => allocation,
        Err(message) => {
            return report_error(
                json_mode,
                &CheckError::operational(message),
                None,
                stdout,
                stderr,
            );
        }
    };
    let started_at = store::timestamp_millis();
    let jobs = args.jobs.unwrap_or_else(default_jobs).max(1);

    let has_nix_checks = selected.iter().any(|id| id.starts_with("nix:"));
    let eligible = context.terminal.progress_eligible(json_mode);
    let mut progress = ProgressReporter::new(
        ProgressReducer::new(selected.len() + 1, has_nix_checks),
        &mut *stderr,
        eligible,
        context.terminal.width(),
        context.terminal.progress_styled(),
    );
    // Render the initialized snapshot before workers start; the reducer is the
    // single source of displayed metrics, so later writes stay change-only.
    progress.start();
    // The formatter gate is serial and mandatory. It runs before any selected
    // check is admitted and short-circuits the run when it fails.
    let FormatGate {
        outcome: format_outcome,
        failure: format_failure,
        operational_error: format_operational_error,
        log: format_log,
    } = run_format_gate(root, backend, &allocation, &interrupt, &mut progress);
    // Source metadata is sampled after formatting so it describes the workspace
    // state the selected checks run against, including any formatter edits.
    let source = store::source_metadata(store.root());
    let execution = if format_outcome == Outcome::Passed {
        execute_selected(
            root,
            &selected,
            &allocation,
            backend,
            &planned.plans,
            &planned.dependencies,
            interrupt.clone(),
            jobs,
            args.fail_fast,
            &mut progress,
        )
    } else {
        // The gate stopped check work. Publish every selected check's final
        // not_run outcome so the progress denominator still completes.
        for id in &selected {
            progress.apply(ProgressEvent::CheckFinalized {
                id: id.clone(),
                outcome: Outcome::NotRun,
            });
        }
        Ok(ExecutionResults {
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
            failures: format_failure.into_iter().collect(),
            operational_error: format_operational_error,
        })
    };
    // Snapshot the last applied tuple before cleanup so the completion summary
    // can repeat it. Metrics are only collected when live progress was eligible.
    let final_metrics = eligible.then(|| progress.final_metrics());
    // Clear the transient line before any summary, error, or interruption
    // report. Cleanup is not a metric event and never repeats a snapshot.
    progress.finish();
    drop(progress);
    let completed_at = store::timestamp_millis();
    let (record, operational_error) = match execution {
        Ok(mut execution) => {
            execution.outcomes.insert(
                FORMAT_WORKSPACE_ID.to_owned(),
                CheckRecord {
                    id: FORMAT_WORKSPACE_ID.to_owned(),
                    outcome: format_outcome,
                    blocked_by: None,
                    logs: format_log.clone().into_iter().collect(),
                },
            );
            let operational_error = execution.operational_error.take();
            let status = if interrupt.is_set() {
                RunStatus::Interrupted
            } else if operational_error.is_some() {
                RunStatus::Error
            } else if execution
                .outcomes
                .values()
                .any(|outcome| outcome.outcome != Outcome::Passed)
            {
                RunStatus::Failed
            } else {
                RunStatus::Passed
            };
            let failures = assign_failure_ids(execution.failures);
            if let Some(blocked) = execution.outcomes.get_mut("static:traceability")
                && blocked.outcome == Outcome::Blocked
            {
                blocked.blocked_by = failures
                    .iter()
                    .find(|failure| failure.check == "static:structure")
                    .map(|failure| failure.id.clone());
            }
            (
                RunRecord {
                    id: allocation.id.clone(),
                    root: store.root().to_string_lossy().into_owned(),
                    started_at,
                    completed_at,
                    source_revision: source.revision.clone(),
                    source_dirty: source.dirty,
                    status,
                    selection,
                    outcomes: execution.outcomes.into_values().collect(),
                    failures,
                    notices: planned.notices,
                },
                operational_error,
            )
        }
        Err(message) => {
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
                logs: format_log.into_iter().collect(),
            });
            outcomes.sort_by(|left, right| left.id.cmp(&right.id));
            let record = RunRecord {
                id: allocation.id.clone(),
                root: store.root().to_string_lossy().into_owned(),
                started_at,
                completed_at,
                source_revision: source.revision.clone(),
                source_dirty: source.dirty,
                status: RunStatus::Error,
                selection,
                outcomes,
                failures: Vec::new(),
                notices: planned.notices,
            };
            if let Err(finalize_error) = store.finalize(&allocation, &record) {
                return report_error(
                    json_mode,
                    &CheckError::operational(format!("{message}; additionally, {finalize_error}")),
                    Some(&allocation.id),
                    stdout,
                    stderr,
                );
            }
            return report_error(
                json_mode,
                &CheckError::operational(message),
                Some(&allocation.id),
                stdout,
                stderr,
            );
        }
    };

    if let Err(message) = store.finalize(&allocation, &record) {
        return report_error(
            json_mode,
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
            json_mode,
            &CheckError::operational(message),
            Some(&record.id),
            stdout,
            stderr,
        );
    }

    let exit_code = match record.status {
        RunStatus::Passed => ExitCode::SUCCESS,
        RunStatus::Failed => ExitCode::from(1),
        RunStatus::Interrupted => ExitCode::from(130),
        RunStatus::Error => ExitCode::from(2),
    };
    let bytes = if json_mode {
        output::run_summary_json(&record).bytes
    } else {
        crate::output::colorize_check(
            &output::run_summary_text(&record, final_metrics.as_deref()),
            crate::output::color_enabled(crate::output::Stream::Stdout, false),
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
    NixBatch {
        log: String,
        result: BatchRealization,
    },
}

enum InitialState {
    StaticPassed,
    StaticFailed(Vec<Diagnostic>),
    Canceled,
    NotRun,
}

#[allow(clippy::too_many_arguments)]
fn execute_selected<W: Write>(
    root: &Path,
    selected: &BTreeSet<String>,
    allocation: &store::RunAllocation,
    backend: &dyn NixBackend,
    plans: &BTreeMap<String, CheckPlan>,
    dependencies: &BTreeMap<String, Vec<String>>,
    interrupt: InterruptFlag,
    jobs: usize,
    fail_fast: bool,
    progress: &mut ProgressReporter<W>,
) -> Result<ExecutionResults, String> {
    let mut outcomes = selected
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
        .collect::<BTreeMap<_, _>>();
    let mut failures = Vec::new();
    let mut operational_error = None;
    let nix_checks = selected
        .iter()
        .filter(|id| id.starts_with("nix:"))
        .cloned()
        .collect::<Vec<_>>();
    let batch_plans = nix_checks
        .iter()
        .filter_map(|id| plans.get(id).cloned())
        .collect::<Vec<_>>();

    let mut initial_tasks: Vec<InitialWork<'_>> = Vec::new();
    let (event_sender, event_receiver) = mpsc::channel();
    let sink = ProgressSink::new(event_sender, progress.is_enabled());
    if selected.contains("static:structure") {
        let root = root.to_path_buf();
        initial_tasks.push((
            "static:structure".to_owned(),
            Box::new(move |_cancellation| InitialTask {
                result: InitialTaskResult::Structure(bloomery_workspace::load(&root).map(Some)),
            }),
        ));
    }
    if !batch_plans.is_empty() {
        let root = root.to_path_buf();
        let allocation = allocation.clone();
        let sink = sink.clone();
        initial_tasks.push((
            "nix:batch".to_owned(),
            Box::new(move |cancellation| {
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
        ));
    }

    let initial_report = run_with_followups(
        initial_tasks,
        jobs,
        interrupt,
        |task| initial_task_stops_admission(task, fail_fast),
        |task| {
            if selected.contains("static:traceability")
                && let InitialTaskResult::Structure(Ok(context)) = &mut task.result
                && let Some(context) = context.take()
            {
                let work: InitialWork<'_> = (
                    "static:traceability".to_owned(),
                    Box::new(move |_cancellation| InitialTask {
                        result: InitialTaskResult::Traceability(bloomery_check::run(&context)),
                    }),
                );
                vec![work]
            } else {
                Vec::new()
            }
        },
        |id, task| {
            if let Some(outcome) = completed_outcome(task) {
                sink.emit(ProgressEvent::CheckFinalized {
                    id: id.to_owned(),
                    outcome,
                });
            }
            // A failed structure prerequisite finalizes traceability as blocked
            // as soon as the failure is known, without waiting for unrelated
            // Nix workers to join.
            if id == "static:structure"
                && matches!(&task.result, InitialTaskResult::Structure(Err(_)))
                && selected.contains("static:traceability")
            {
                sink.emit(ProgressEvent::CheckFinalized {
                    id: "static:traceability".to_owned(),
                    outcome: Outcome::Blocked,
                });
            }
        },
        |queued: &[String]| {
            // Queued work can never run once admission is permanently stopped;
            // publish its final not_run outcomes while active workers join.
            for id in queued {
                sink.emit(ProgressEvent::CheckFinalized {
                    id: id.clone(),
                    outcome: Outcome::NotRun,
                });
            }
        },
        |tracker: &WorkerTracker| {
            // Drain on the caller thread so a single owner performs all
            // progress writes. Delivery failure never stops check work.
            while !tracker.wait_timeout(Duration::from_millis(5)) {
                while let Ok(event) = event_receiver.try_recv() {
                    progress.apply(event);
                }
            }
            while let Ok(event) = event_receiver.try_recv() {
                progress.apply(event);
            }
        },
    );
    let mut initial = BTreeMap::new();
    let mut batch_log: Option<String> = None;
    let mut batch = BatchRealization::default();
    let mut batch_canceled = false;
    for result in initial_report.results {
        let id = result.id;
        if id == "nix:batch" {
            match result.state {
                ScheduledState::Completed(task) => {
                    if let InitialTaskResult::NixBatch { log, result } = task.result {
                        batch_log = Some(log);
                        batch = result;
                    }
                }
                ScheduledState::Canceled => batch_canceled = true,
                ScheduledState::NotRun => {}
            }
            continue;
        }
        let state = match result.state {
            ScheduledState::Completed(task) => match task.result {
                InitialTaskResult::Structure(Ok(_)) | InitialTaskResult::Traceability(Ok(())) => {
                    InitialState::StaticPassed
                }
                InitialTaskResult::Structure(Err(mut diagnostics))
                | InitialTaskResult::Traceability(Err(mut diagnostics)) => {
                    sort_model_diagnostics(&mut diagnostics);
                    InitialState::StaticFailed(diagnostics)
                }
                InitialTaskResult::NixBatch { .. } => continue,
            },
            ScheduledState::Canceled => InitialState::Canceled,
            ScheduledState::NotRun => InitialState::NotRun,
        };
        initial.insert(id, state);
    }

    if batch.canceled {
        batch_canceled = true;
    }
    if let Some(message) = batch.operational_error.clone() {
        operational_error.get_or_insert(message);
    }

    // Resolve every selected derivation's output validity in one query.
    let output_paths = nix_checks
        .iter()
        .filter_map(|id| plans.get(id))
        .flat_map(|plan| plan.outputs.iter().cloned())
        .collect::<Vec<_>>();
    let valid = if output_paths.is_empty() {
        BTreeSet::new()
    } else {
        match backend.validate_outputs(root, &output_paths) {
            Ok(valid) => valid,
            Err(message) => {
                operational_error.get_or_insert(message);
                BTreeSet::new()
            }
        }
    };

    for (id, state) in initial {
        match state {
            InitialState::StaticPassed => {
                outcomes
                    .get_mut(&id)
                    .expect("selected static structure")
                    .outcome = Outcome::Passed;
            }
            InitialState::StaticFailed(diagnostics) => {
                outcomes
                    .get_mut(&id)
                    .expect("selected static structure")
                    .outcome = Outcome::Failed;
                failures.extend(diagnostic_failures(root, &id, diagnostics, None));
            }
            InitialState::Canceled => {
                outcomes.get_mut(&id).expect("selected task").outcome = Outcome::Canceled;
            }
            InitialState::NotRun => {
                outcomes.get_mut(&id).expect("selected task").outcome = Outcome::NotRun;
            }
        }
    }

    // Map the single batched realization outcome onto every selected Nix check.
    // A check is passed only when all of its derivation outputs are valid; a
    // failed or causally-blocked derivation is failed; fail-fast cancellation
    // leaves incomplete checks canceled and never-started ones not_run.
    for id in &nix_checks {
        let Some(plan) = plans.get(id) else {
            continue;
        };
        let outcome = outcomes.get_mut(id).expect("selected Nix check");
        if let Some(log) = &batch_log {
            outcome.logs.push(log.clone());
        }
        if let Some(error) = &plan.eval_error {
            outcome.outcome = Outcome::Failed;
            failures.push(FailureDraft {
                check: id.clone(),
                code: error.code.clone(),
                subject: None,
                location: None,
                message: error.message.clone(),
                notes: Vec::new(),
                log: batch_log.clone(),
                nix_log: None,
                focus_tail: false,
                occurrence: failures.len(),
            });
            continue;
        }
        let Some(derivation) = plan.derivation.clone() else {
            outcome.outcome = Outcome::NotRun;
            continue;
        };
        let all_valid =
            !plan.outputs.is_empty() && plan.outputs.iter().all(|output| valid.contains(output));
        let own_block = batch.failures.get(&derivation);
        // A dependency-only failure block has no builder log of its own; prefer
        // the causal failed dependency so details can resolve a useful log.
        let dependency_cause = own_block
            .is_none_or(|block| block.log_hint.is_none())
            .then(|| causal_failure(&derivation, &batch.failures, dependencies))
            .flatten();
        if let Some(cause) = dependency_cause {
            outcome.outcome = Outcome::Failed;
            failures.push(failure_from_block(
                id,
                &cause,
                batch_log.clone(),
                cause.derivation.clone(),
                failures.len(),
            ));
            continue;
        }
        if let Some(block) = own_block {
            outcome.outcome = Outcome::Failed;
            failures.push(failure_from_block(
                id,
                block,
                batch_log.clone(),
                derivation.clone(),
                failures.len(),
            ));
            continue;
        }
        if all_valid {
            outcome.outcome = Outcome::Passed;
        } else if batch_canceled {
            outcome.outcome = Outcome::Canceled;
        } else {
            outcome.outcome = Outcome::NotRun;
        }
    }

    if selected.contains("static:traceability")
        && outcomes
            .get("static:structure")
            .is_some_and(|structure| structure.outcome == Outcome::Failed)
    {
        outcomes
            .get_mut("static:traceability")
            .expect("selected traceability check")
            .outcome = Outcome::Blocked;
    }

    // Every selected check has a final state even when fail-fast or interruption
    // prevented admission of a later-stage task.
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
    for outcome in outcomes.values_mut() {
        outcome.logs.sort();
        outcome.logs.dedup();
    }

    // Reconcile every selected ID. Final transitions were published live when
    // known; this deduplicated pass is the safety net for any outcome that was
    // only determined after its worker joined.
    for (id, record) in &outcomes {
        sink.emit(ProgressEvent::CheckFinalized {
            id: id.clone(),
            outcome: record.outcome,
        });
    }
    while let Ok(event) = event_receiver.try_recv() {
        progress.apply(event);
    }

    Ok(ExecutionResults {
        outcomes,
        failures,
        operational_error,
    })
}

fn completed_outcome(task: &InitialTask) -> Option<Outcome> {
    match &task.result {
        InitialTaskResult::Structure(Ok(_)) | InitialTaskResult::Traceability(Ok(())) => {
            Some(Outcome::Passed)
        }
        InitialTaskResult::Structure(Err(_)) | InitialTaskResult::Traceability(Err(_)) => {
            Some(Outcome::Failed)
        }
        InitialTaskResult::NixBatch { .. } => None,
    }
}

fn initial_task_stops_admission(task: &InitialTask, fail_fast: bool) -> bool {
    match &task.result {
        InitialTaskResult::Structure(Err(_)) | InitialTaskResult::Traceability(Err(_)) => fail_fast,
        InitialTaskResult::NixBatch { result, .. } => {
            result.operational_error.is_some() || (fail_fast && !result.failures.is_empty())
        }
        InitialTaskResult::Structure(Ok(_)) | InitialTaskResult::Traceability(Ok(())) => false,
    }
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
