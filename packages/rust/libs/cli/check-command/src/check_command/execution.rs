use super::catalog::{NixBackend, NixTaskResult, nix_log_store_path, parse_nix_id};
use super::command::{CheckArgs, CheckContext, CheckError, discover_for_selection, report_error};
use super::interrupt::{CancellationToken, InterruptFlag};
use super::model::{
    CheckRecord, FailureLocation, FailureRecord, Outcome, RunRecord, RunSelection, RunStatus,
    select_ids,
};
use super::output;
use super::scheduler::{ScheduledState, run_with_followups};
use super::store;
use bloomery_model::{Diagnostic, sort_diagnostics as sort_model_diagnostics};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

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
    let catalog = match discover_for_selection(root, &args.selectors, &args.systems, backend) {
        Ok(catalog) => catalog,
        Err(error) => return report_error(json_mode, &error, None, stdout, stderr),
    };
    let direct_selection = match select_ids(&catalog.ids, &args.selectors) {
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
        systems: catalog.systems.clone(),
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
    let source = store::source_metadata(store.root());
    let started_at = store::timestamp_millis();
    let jobs = args.jobs.unwrap_or_else(default_jobs).max(1);

    let execution = execute_selected(
        root,
        &selected,
        &allocation,
        backend,
        interrupt.clone(),
        jobs,
        args.fail_fast,
    );
    let completed_at = store::timestamp_millis();
    let (record, operational_error) = match execution {
        Ok(mut execution) => {
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
                    notices: catalog.notices,
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
                notices: catalog.notices,
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
            &output::run_summary_text(&record),
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

#[derive(Debug, Clone)]
struct CatalogSelection {
    id: String,
    system: String,
    attribute: String,
}

type InitialWork<'a> = (
    String,
    Box<dyn FnOnce(CancellationToken) -> InitialTask + Send + 'a>,
);

struct InitialTask {
    result: InitialTaskResult,
}

enum InitialTaskResult {
    Structure(Result<Option<bloomery_model::Context>, Vec<Diagnostic>>),
    Traceability(Result<(), Vec<Diagnostic>>),
    NixCheck {
        log: String,
        result: NixTaskResult<()>,
    },
}

enum InitialState {
    StaticPassed,
    StaticFailed(Vec<Diagnostic>),
    NixCheck {
        log: String,
        result: NixTaskResult<()>,
    },
    Canceled,
    NotRun,
}

fn execute_selected(
    root: &Path,
    selected: &BTreeSet<String>,
    allocation: &store::RunAllocation,
    backend: &dyn NixBackend,
    interrupt: InterruptFlag,
    jobs: usize,
    fail_fast: bool,
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
        .filter_map(|id| {
            parse_nix_id(id).map(|(system, attribute)| CatalogSelection {
                id: id.clone(),
                system: system.to_owned(),
                attribute: attribute.to_owned(),
            })
        })
        .collect::<Vec<_>>();

    let mut initial_tasks: Vec<InitialWork<'_>> = Vec::new();
    if selected.contains("static:structure") {
        let root = root.to_path_buf();
        initial_tasks.push((
            "static:structure".to_owned(),
            Box::new(move |_cancellation| InitialTask {
                result: InitialTaskResult::Structure(bloomery_workspace::load(&root).map(Some)),
            }),
        ));
    }
    for (index, check) in nix_checks.iter().enumerate() {
        let check = check.clone();
        let root = root.to_path_buf();
        let allocation = allocation.clone();
        initial_tasks.push((
            check.id.clone(),
            Box::new(move |cancellation| {
                let log_name = format!("nix-check-{index:04}.log");
                let result = match allocation.prepare_log(&log_name) {
                    Ok(log_path) => backend.realize_check(
                        &root,
                        &check.system,
                        &check.attribute,
                        &log_path,
                        cancellation,
                    ),
                    Err(message) => NixTaskResult::OperationalError(message),
                };
                InitialTask {
                    result: InitialTaskResult::NixCheck {
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
    );
    let mut initial = BTreeMap::new();
    for result in initial_report.results {
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
                InitialTaskResult::NixCheck { log, result } => {
                    InitialState::NixCheck { log, result }
                }
            },
            ScheduledState::Canceled => InitialState::Canceled,
            ScheduledState::NotRun => InitialState::NotRun,
        };
        initial.insert(result.id, state);
    }

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
            InitialState::NixCheck { log, result } => {
                let outcome = outcomes.get_mut(&id).expect("selected Nix check");
                outcome.logs.push(log.clone());
                match result {
                    NixTaskResult::Succeeded(()) => outcome.outcome = Outcome::Passed,
                    NixTaskResult::Failed { code, message } => {
                        outcome.outcome = Outcome::Failed;
                        let nix_log = retained_nix_log_path(&allocation.directory, &log);
                        failures.push(FailureDraft {
                            check: id,
                            code,
                            subject: None,
                            location: None,
                            message,
                            notes: Vec::new(),
                            log: Some(log),
                            nix_log,
                            focus_tail: true,
                            occurrence: failures.len(),
                        });
                    }
                    NixTaskResult::Canceled => outcome.outcome = Outcome::Canceled,
                    NixTaskResult::OperationalError(message) => {
                        outcome.outcome = Outcome::Canceled;
                        operational_error.get_or_insert(message);
                    }
                }
            }
            InitialState::Canceled => {
                outcomes.get_mut(&id).expect("selected task").outcome = Outcome::Canceled;
            }
            InitialState::NotRun => {
                outcomes.get_mut(&id).expect("selected task").outcome = Outcome::NotRun;
            }
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

    Ok(ExecutionResults {
        outcomes,
        failures,
        operational_error,
    })
}

fn initial_task_stops_admission(task: &InitialTask, fail_fast: bool) -> bool {
    match &task.result {
        InitialTaskResult::Structure(Err(_)) | InitialTaskResult::Traceability(Err(_)) => fail_fast,
        InitialTaskResult::NixCheck { result, .. } => match result {
            NixTaskResult::Failed { .. } => fail_fast,
            NixTaskResult::OperationalError(_) => true,
            NixTaskResult::Succeeded(()) | NixTaskResult::Canceled => false,
        },
        InitialTaskResult::Structure(Ok(_)) | InitialTaskResult::Traceability(Ok(())) => false,
    }
}

fn retained_nix_log_path(run_directory: &Path, log: &str) -> Option<String> {
    let path = store::resolve_log_path(run_directory, log).ok()?;
    let output = std::fs::read(path).ok()?;
    nix_log_store_path(&output)
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
