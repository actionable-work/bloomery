use crate::check_command::command::{
    CheckArgs, CheckContext, CheckOperation, ListArgs, run_at_with,
};
use crate::check_command::interrupt::InterruptFlag;
use crate::check_command::model::Outcome;
use crate::check_command::progress::{
    ProgressEvent, ProgressReducer, ProgressReporter, ProgressSink,
};
use crate::check_command::scheduler::{Task, WorkerTracker, run_with_followups};
use crate::check_command::test_support::{FakeNix, fixture, invoke_with_terminal, request};
use crate::output::TerminalFacts;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

fn tty() -> TerminalFacts {
    TerminalFacts {
        stderr_is_terminal: true,
        columns: Some("200".to_owned()),
        ..TerminalFacts::default()
    }
}

fn stdio_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[derive(Clone)]
struct SharedWriter(Arc<Mutex<Vec<u8>>>);

impl Write for SharedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .expect("shared writer")
            .extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-001"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-009"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-011"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-012"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-024"))]
fn eligible_execution_writes_changing_metrics_to_stderr_and_final_metrics_to_stdout() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["alias-a".to_owned(), "alias-b".to_owned()],
        scripted_events: Mutex::new(vec![
            crate::check_command::progress::DerivationEvent::BuildStarted {
                drv: "/nix/store/shared.drv".to_owned(),
            },
        ]),
        ..FakeNix::default()
    };
    let mut args = request();
    args.selectors = vec!["nix:*".to_owned()];
    args.jobs = Some(2);
    let (status, stdout, stderr) = invoke_with_terminal(
        args,
        &root,
        &cache,
        &backend,
        false,
        InterruptFlag::for_test(),
        tty(),
    );
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let stderr = stdio_text(&stderr);
    let stdout = stdio_text(&stdout);

    // Two alias checks and the format:workspace gate complete as separate
    // units while the shared derivation contributes a single work unit.
    assert!(stderr.contains("checks 3/3 complete"), "stderr: {stderr:?}");
    assert!(stderr.contains("built 1"), "stderr: {stderr:?}");
    assert!(stderr.contains("known total 1"), "stderr: {stderr:?}");
    // Live progress is stderr-only. The completion summary closes with the
    // compact final work metrics, without the pending counters or check counter.
    assert!(!stdout.contains("checks "), "stdout: {stdout:?}");
    assert!(stdout.contains("built 1"), "stdout: {stdout:?}");
    assert!(stdout.contains("total 1"), "stdout: {stdout:?}");
    assert!(!stdout.contains("to build"), "stdout: {stdout:?}");
    assert!(!stdout.contains("to fetch"), "stdout: {stdout:?}");
    assert!(stdout.contains("passed"));
    // The initialized snapshot is the first progress write, before any worker
    // event changes a metric.
    assert!(
        stderr.starts_with("\r\u{1b}[K\u{1b}[1mchecks 0/3"),
        "stderr: {stderr:?}"
    );
    // The transient line is explicitly cleared before final reporting.
    assert!(
        stderr.trim_end().ends_with("\u{1b}[K"),
        "stderr: {stderr:?}"
    );

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-030"))]
fn eligible_completion_summary_repeats_the_final_metrics() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["check".to_owned()],
        ..FakeNix::default()
    };
    let (status, stdout, _) = invoke_with_terminal(
        request(),
        &root,
        &cache,
        &backend,
        false,
        InterruptFlag::for_test(),
        tty(),
    );
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let stdout = stdio_text(&stdout);
    assert!(!stdout.contains("checks "), "stdout: {stdout:?}");
    assert!(stdout.contains("run "), "stdout: {stdout:?}");
    // The final work metrics close the message rather than trailing the status
    // line like the live progress line.
    let last = stdout.trim_end().lines().last().unwrap_or_default();
    assert!(last.contains("total"), "stdout: {stdout:?}");
    assert!(!last.contains("to build"), "stdout: {stdout:?}");
    assert!(!last.contains("to fetch"), "stdout: {stdout:?}");

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-031"))]
fn completion_output_omits_metrics_without_a_user_terminal() {
    // Redirected stderr in human mode.
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["check".to_owned()],
        ..FakeNix::default()
    };
    let (status, stdout, _) = invoke_with_terminal(
        request(),
        &root,
        &cache,
        &backend,
        false,
        InterruptFlag::for_test(),
        TerminalFacts::default(),
    );
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let stdout = stdio_text(&stdout);
    assert!(stdout.contains("passed"), "stdout: {stdout:?}");
    assert!(!stdout.contains("checks "), "stdout: {stdout:?}");
    assert!(!stdout.contains("to fetch"), "stdout: {stdout:?}");
    assert!(!stdout.contains("built "), "stdout: {stdout:?}");
    assert!(!stdout.contains("cached "), "stdout: {stdout:?}");

    // JSON mode suppresses the completion metrics even on a terminal.
    let (status, stdout, _) = invoke_with_terminal(
        request(),
        &root,
        &cache,
        &backend,
        true,
        InterruptFlag::for_test(),
        tty(),
    );
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let stdout = stdio_text(&stdout);
    assert!(!stdout.contains("checks "), "stdout: {stdout:?}");
    assert!(!stdout.contains("built "), "stdout: {stdout:?}");
    assert!(!stdout.contains("cached "), "stdout: {stdout:?}");

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-002"))]
fn non_terminal_stderr_receives_no_progress_on_either_stream() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["check".to_owned()],
        ..FakeNix::default()
    };
    let (status, stdout, stderr) = invoke_with_terminal(
        request(),
        &root,
        &cache,
        &backend,
        true,
        InterruptFlag::for_test(),
        TerminalFacts::default(),
    );
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    assert!(!stdio_text(&stderr).contains('\u{1b}'));
    assert!(!stdio_text(&stderr).contains("checks "));
    assert!(!stdio_text(&stdout).contains("checks "));
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-003"))]
fn ci_markers_suppress_progress_even_with_a_terminal() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["check".to_owned()],
        ..FakeNix::default()
    };
    let mut terminal = tty();
    terminal.ci = Some("1".to_owned());
    let (status, _, stderr) = invoke_with_terminal(
        request(),
        &root,
        &cache,
        &backend,
        true,
        InterruptFlag::for_test(),
        terminal,
    );
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    assert!(!stdio_text(&stderr).contains("checks "));
    assert!(!stdio_text(&stderr).contains('\u{1b}'));
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-004"))]
fn json_mode_suppresses_progress_on_both_streams() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["check".to_owned()],
        ..FakeNix::default()
    };
    let (status, stdout, stderr) = invoke_with_terminal(
        request(),
        &root,
        &cache,
        &backend,
        true,
        InterruptFlag::for_test(),
        tty(),
    );
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    assert!(!stdio_text(&stderr).contains('\u{1b}'));
    assert!(!stdio_text(&stderr).contains("checks "));
    let stdout = stdio_text(&stdout);
    assert!(stdout.trim_start().starts_with('{'));
    assert!(!stdout.contains("checks "));
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-005"))]
fn dumb_terminals_suppress_progress() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["check".to_owned()],
        ..FakeNix::default()
    };
    let mut terminal = tty();
    terminal.term = Some("dumb".to_owned());
    let (status, _, stderr) = invoke_with_terminal(
        request(),
        &root,
        &cache,
        &backend,
        true,
        InterruptFlag::for_test(),
        terminal,
    );
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    assert!(!stdio_text(&stderr).contains("checks "));
    assert!(!stdio_text(&stderr).contains('\u{1b}'));
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-006"))]
fn no_color_keeps_progress_without_styling() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["check".to_owned()],
        ..FakeNix::default()
    };
    let mut terminal = tty();
    terminal.no_color = Some("1".to_owned());
    let (status, _, stderr) = invoke_with_terminal(
        request(),
        &root,
        &cache,
        &backend,
        false,
        InterruptFlag::for_test(),
        terminal,
    );
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let stderr = stdio_text(&stderr);
    assert!(stderr.contains("checks "));
    assert!(!stderr.contains("\u{1b}[1m"));
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-007"))]
fn catalog_and_retrieval_subcommands_never_start_progress() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["check".to_owned()],
        ..FakeNix::default()
    };
    let args = CheckArgs {
        operation: Some(CheckOperation::List(ListArgs::default())),
        ..CheckArgs::default()
    };
    let (status, _, stderr) = invoke_with_terminal(
        args,
        &root,
        &cache,
        &backend,
        false,
        InterruptFlag::for_test(),
        tty(),
    );
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    assert!(!stdio_text(&stderr).contains('\u{1b}'));
    assert!(!stdio_text(&stderr).contains("checks "));
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-019"))]
fn all_store_hits_report_known_zero_work() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["check".to_owned()],
        ..FakeNix::default()
    };
    let (status, _, stderr) = invoke_with_terminal(
        request(),
        &root,
        &cache,
        &backend,
        false,
        InterruptFlag::for_test(),
        tty(),
    );
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let stderr = stdio_text(&stderr);
    assert!(stderr.contains("built 0"), "stderr: {stderr:?}");
    assert!(stderr.contains("known total 0"), "stderr: {stderr:?}");
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-021"))]
fn static_only_runs_report_zero_derivation_work() {
    let (root, cache) = fixture(true);
    let backend = FakeNix::default();
    let (status, _, stderr) = invoke_with_terminal(
        request(),
        &root,
        &cache,
        &backend,
        false,
        InterruptFlag::for_test(),
        tty(),
    );
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let stderr = stdio_text(&stderr);
    assert!(stderr.contains("built 0"), "stderr: {stderr:?}");
    assert!(stderr.contains("known total 0"), "stderr: {stderr:?}");
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-028"))]
fn final_reporting_clears_the_transient_line_before_any_summary() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["check".to_owned()],
        ..FakeNix::default()
    };
    let (status, _, stderr) = invoke_with_terminal(
        request(),
        &root,
        &cache,
        &backend,
        false,
        InterruptFlag::for_test(),
        tty(),
    );
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let stderr = stdio_text(&stderr);
    assert!(stderr.contains("\u{1b}[K"));
    // The last bytes are the clean-line erase, and the unchanged final snapshot
    // is not repeated after it.
    let last_clear = stderr.rfind("\u{1b}[K").expect("cleanup erase");
    assert!(stderr[last_clear..].len() <= "\u{1b}[K".len() + 1);
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-009"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-028"))]
fn blocked_dependents_finalize_before_a_slow_worker_joins() {
    let (root, cache) = fixture(true);
    std::fs::remove_file(root.join(".bloomery/specs/CLI/CHECK/README.md"))
        .expect("remove the static prerequisite");
    let release = Arc::new(std::sync::Barrier::new(2));
    let backend = FakeNix {
        names: vec!["slow".to_owned()],
        hold_until_released: Some(release.clone()),
        ..FakeNix::default()
    };
    let observed = Arc::new(Mutex::new(Vec::<u8>::new()));
    let writer = SharedWriter(observed.clone());
    let root_for_run = root.clone();
    let cache_for_run = cache.clone();
    let run = std::thread::spawn(move || {
        let mut stdout = Vec::new();
        let mut stderr = writer;
        run_at_with(
            request(),
            CheckContext {
                root: &root_for_run,
                json_mode: false,
                backend: &backend,
                interrupt: InterruptFlag::for_test(),
                cache_base: Some(&cache_for_run),
                terminal: tty(),
            },
            &mut stdout,
            &mut stderr,
        )
    });

    // Structure failure blocks traceability immediately, so structure,
    // traceability, and the formatter gate are all complete while the held Nix
    // worker is still active.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut blocked_published = false;
    while Instant::now() < deadline {
        if stdio_text(&observed.lock().expect("shared writer")).contains("checks 3/4") {
            blocked_published = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    release.wait();
    let status = run.join().expect("check runner thread");
    let stderr = stdio_text(&observed.lock().expect("shared writer"));
    assert!(blocked_published, "stderr: {stderr:?}");
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-009"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-027"))]
fn finalization_is_published_while_another_worker_is_still_running() {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let (release_sender, release_receiver) = mpsc::channel::<()>();
    let published = Arc::new(AtomicBool::new(false));
    let published_flag = published.clone();
    let observed = buffer.clone();

    let worker = std::thread::spawn(move || {
        let (sender, receiver) = mpsc::channel();
        let sink = ProgressSink::new(sender, true);
        let mut reporter = ProgressReporter::new(
            ProgressReducer::new(2, false),
            SharedWriter(buffer.clone()),
            true,
            200,
            false,
        );
        let tasks: Vec<(String, Task<'_, &'static str>)> = vec![
            ("fast".to_owned(), Box::new(|_| "fast")),
            (
                "slow".to_owned(),
                Box::new(move |_| {
                    let _ = release_receiver.recv();
                    "slow"
                }),
            ),
        ];
        run_with_followups(
            tasks,
            2,
            InterruptFlag::for_test(),
            |_| false,
            |_| Vec::new(),
            |id, _result| {
                sink.emit(ProgressEvent::CheckFinalized {
                    id: id.to_owned(),
                    outcome: Outcome::Passed,
                });
            },
            |_queued| {},
            |tracker: &WorkerTracker| {
                while !tracker.wait_timeout(Duration::from_millis(2)) {
                    while let Ok(event) = receiver.try_recv() {
                        reporter.apply(event);
                    }
                }
                while let Ok(event) = receiver.try_recv() {
                    reporter.apply(event);
                }
            },
        );
    });

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let text = stdio_text(&observed.lock().expect("shared writer"));
        if text.contains("checks 1/2") {
            published_flag.store(true, Ordering::SeqCst);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "completion was not published before the remaining worker joined"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    release_sender.send(()).expect("release slow worker");
    worker.join().expect("scheduler thread");
    assert!(published.load(Ordering::SeqCst));
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-009"))]
fn formatter_failure_finalizes_every_selected_check() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        formatter_result: Some(crate::check_command::catalog::NixTaskResult::Failed {
            code: "NixFormatFailed".to_owned(),
            message: "nix fmt failed".to_owned(),
        }),
        ..FakeNix::default()
    };
    let (status, stdout, stderr) = invoke_with_terminal(
        request(),
        &root,
        &cache,
        &backend,
        false,
        InterruptFlag::for_test(),
        tty(),
    );
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let stderr = stdio_text(&stderr);
    // The formatter gate plus both selected static checks reach a final
    // outcome, so the fixed denominator completes even though no check ran.
    assert!(stderr.contains("checks 1/3"), "stderr: {stderr:?}");
    assert!(stderr.contains("checks 3/3 complete"), "stderr: {stderr:?}");
    assert!(stdio_text(&stdout).contains("failed"));
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(cache);
}
