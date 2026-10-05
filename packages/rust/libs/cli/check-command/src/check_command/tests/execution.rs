use crate::check_command::catalog::{NixBackend, NixCli, NixTaskResult};
use crate::check_command::command::{
    CheckContext, CheckOperation, DetailsArgs, FailureArgs, ListArgs, run_at_with,
};
use crate::check_command::execution::{FailureDraft, assign_failure_ids, default_jobs};
use crate::check_command::interrupt::InterruptFlag;
use crate::check_command::model::{Outcome, RunStatus};
use crate::check_command::store::RunStore;
use crate::check_command::test_support::{
    FakeNix, fixture, invoke, invoke_with_interrupt, lock_fixture_flake, nix_is_available, request,
};
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::Ordering;

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-001"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-007"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-008"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-018"))]
fn traceability_failure_stops_nix_work_without_waiting_for_builds() {
    let (root, cache) = fixture(true);
    let requirement = root.join(".bloomery/specs/CLI/CHECK/requirements/CONTRACT.toml");
    let contents = fs::read_to_string(&requirement).expect("fixture requirement");
    fs::write(
        &requirement,
        contents.replace("manual = true", "manual = false"),
    )
    .expect("automated requirement without evidence");
    let backend = FakeNix {
        names: vec!["running".to_owned(), "queued".to_owned()],
        wait_for_cancellation: true,
        ..FakeNix::default()
    };
    let mut args = request();
    args.jobs = Some(2);
    args.fail_fast = true;
    let (status, _, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let (record, _) = RunStore::open(&root, Some(&cache))
        .unwrap()
        .latest()
        .unwrap();
    assert_eq!(record.failures.len(), 1);
    assert_eq!(record.failures[0].check, "static:traceability");
    let nix = record
        .outcomes
        .iter()
        .filter(|outcome| outcome.id.starts_with("nix:"))
        .collect::<Vec<_>>();
    assert!(
        nix.iter()
            .all(|outcome| matches!(outcome.outcome, Outcome::Canceled | Outcome::NotRun))
    );
    assert!(nix.iter().any(|outcome| outcome.outcome == Outcome::NotRun));
    assert!(backend.build_commands.load(Ordering::SeqCst) <= 1);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-DETAIL-024"))]
fn failed_nix_checks_retain_referenced_derivation_log_paths() {
    let (root, cache) = fixture(true);
    let store_path = "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-failed-check.drv";
    let backend = FakeNix {
        names: vec!["broken".to_owned()],
        realization_result: Some(NixTaskResult::Failed {
            code: "NixCheckFailed".to_owned(),
            message: "build failed".to_owned(),
        }),
        log_contents: Some(format!("For full logs, run: nix log {store_path}\n").into_bytes()),
        ..FakeNix::default()
    };
    let (status, summary, _) = invoke(request(), &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let summary: Value = serde_json::from_slice(&summary).expect("check summary");
    let run_id = summary["run"].as_str().expect("run ID");
    let store = RunStore::open(&root, Some(&cache)).expect("run store");
    let (record, _) = store.load(run_id).expect("retained run");

    assert_eq!(record.failures.len(), 1);
    assert_eq!(record.failures[0].nix_log.as_deref(), Some(store_path));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-007"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-008"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-009"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-OUTPUT-011"))]
fn aliased_check_build_requests_rely_on_native_store_deduplication() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["alias-a".to_owned(), "alias-b".to_owned()],
        ..FakeNix::default()
    };
    let mut args = request();
    args.selectors = vec!["nix:*".to_owned()];
    let (status, stdout, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    assert_eq!(backend.evaluations.load(Ordering::SeqCst), 2);
    assert_eq!(backend.build_commands.load(Ordering::SeqCst), 2);
    assert_eq!(backend.realizations.load(Ordering::SeqCst), 1);
    let result: Value = serde_json::from_slice(&stdout).expect("summary");
    assert_eq!(result["counts"]["passed"], 3);
    assert_eq!(result["total"], 0);
    let store =
        crate::check_command::store::RunStore::open(&root, Some(&cache)).expect("retained store");
    let retained = store.latest().expect("latest run").0;
    assert!(retained.outcomes.iter().any(|outcome| {
        outcome.id == "nix:x86_64-linux:alias-a" && outcome.outcome == Outcome::Passed
    }));
    assert!(retained.outcomes.iter().any(|outcome| {
        outcome.id == "nix:x86_64-linux:alias-b" && outcome.outcome == Outcome::Passed
    }));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-007"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-008"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-009"))]
fn real_nix_runner_builds_two_aliases_of_one_low_storage_derivation() {
    if !nix_is_available() {
        eprintln!("skipping real-Nix alias test: Nix store is unavailable");
        return;
    }

    let (root, cache) = fixture(true);
    let backend = NixCli::default();
    let system = backend.host_system(&root).expect("host Nix system");
    let flake = format!(
        r#"{{
  description = "bloomery alias fixture";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = {{ self, nixpkgs }}:
    let
      pkgs = nixpkgs.legacyPackages."{system}";
      formatter = pkgs.writeShellScriptBin "noop-formatter" "exit 0";
      shared = builtins.derivation {{
        name = "bloomery-aliased-check";
        system = "{system}";
        builder = "/bin/sh";
        args = [ "-c" "echo passed > $out" ];
      }};
    in {{
      formatter."{system}" = formatter;
      checks."{system}" = {{
        alias-a = shared;
        alias-b = shared;
      }};
    }};
}}"#
    );
    fs::write(root.join("flake.nix"), flake).expect("write alias fixture");
    lock_fixture_flake(&root);
    let flake_lock_before = fs::read(root.join("flake.lock")).expect("fixture lock");

    let derivation_path = |attribute: &str| {
        let installable = format!(".#checks.{system}.{attribute}.drvPath");
        let output = Command::new("nix")
            .args([
                "eval",
                "--raw",
                "--no-write-lock-file",
                "--no-update-lock-file",
            ])
            .arg(installable)
            .current_dir(&root)
            .output()
            .expect("evaluate derivation path");
        assert!(
            output.status.success(),
            "unable to evaluate {attribute} derivation: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("derivation path UTF-8")
            .trim()
            .to_owned()
    };
    assert_eq!(derivation_path("alias-a"), derivation_path("alias-b"));

    let mut args = request();
    args.selectors = vec!["nix:*".to_owned()];
    let (status, stdout, stderr) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(
        status,
        std::process::ExitCode::SUCCESS,
        "runner stderr: {}",
        String::from_utf8_lossy(&stderr)
    );
    let summary: Value = serde_json::from_slice(&stdout).expect("runner summary");
    assert_eq!(summary["counts"]["passed"], 3);

    let store = RunStore::open(&root, Some(&cache)).expect("retained store");
    let retained = store.latest().expect("retained run").0;
    for attribute in ["alias-a", "alias-b"] {
        assert!(retained.outcomes.iter().any(|outcome| {
            outcome.id == format!("nix:{system}:{attribute}") && outcome.outcome == Outcome::Passed
        }));
    }
    assert_eq!(
        fs::read(root.join("flake.lock")).expect("fixture lock"),
        flake_lock_before
    );

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-005"))]
fn default_execution_attempts_other_independent_checks_after_a_build_failure() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["failed".to_owned(), "independent".to_owned()],
        unique_derivations: true,
        realization_result: Some(NixTaskResult::Failed {
            code: "BuildFailed".to_owned(),
            message: "build failed".to_owned(),
        }),
        ..FakeNix::default()
    };
    let mut args = request();
    args.selectors = vec!["nix:*".to_owned()];
    args.jobs = Some(1);
    let (status, stdout, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    assert_eq!(backend.realizations.load(Ordering::SeqCst), 2);
    let result: Value = serde_json::from_slice(&stdout).expect("summary");
    assert_eq!(result["counts"]["failed"], 2);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-004"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-011"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-017"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-004"))]
fn default_jobs_are_positive_task_output_isolated_and_locks_are_unchanged() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["one".to_owned(), "two".to_owned()],
        formatter_edits: vec![("formatted-source.rs".to_owned(), b"// formatted\n".to_vec())],
        ..FakeNix::default()
    };
    assert!(default_jobs() >= 1);
    let lock_files = ["Cargo.lock", "bloomery.lock", "flake.lock"]
        .into_iter()
        .map(|name| {
            let path = root.join(name);
            fs::write(&path, format!("unchanged {name}\n")).expect("lock fixture");
            let contents = fs::read(&path).expect("lock before run");
            (path, contents)
        })
        .collect::<Vec<_>>();
    let (status, stdout, _) = invoke(request(), &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    assert_eq!(backend.evaluations.load(Ordering::SeqCst), 2);
    // Formatter edits persist, while repository lockfiles are untouched.
    assert_eq!(
        fs::read(root.join("formatted-source.rs")).expect("formatter edit"),
        b"// formatted\n"
    );
    for (path, before) in lock_files {
        assert_eq!(fs::read(&path).expect("lock after run"), before);
    }
    let output = String::from_utf8(stdout).expect("JSON summary");
    assert!(!output.contains("build log output"));
    let store =
        crate::check_command::store::RunStore::open(&root, Some(&cache)).expect("retained store");
    let (retained, directory) = store.latest().expect("latest run");
    let evaluation_logs = retained
        .outcomes
        .iter()
        .flat_map(|outcome| outcome.logs.iter())
        .filter(|log| log.contains("nix-check"))
        .collect::<Vec<_>>();
    assert_eq!(evaluation_logs.len(), 2);
    let contents = evaluation_logs
        .iter()
        .map(|log| {
            let path = crate::check_command::store::resolve_log_path(&directory, log)
                .expect("safe retained log path");
            fs::read_to_string(path).expect("task-isolated evaluation log")
        })
        .collect::<Vec<_>>();
    assert!(contents.iter().any(|log| log.contains("one")));
    assert!(contents.iter().any(|log| log.contains("two")));
    assert!(backend.maximum_active.load(Ordering::SeqCst) >= 1);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-010"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-014"))]
fn evaluation_failures_are_check_failures_and_store_failures_are_operational() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["bad-evaluation".to_owned()],
        realization_result: Some(NixTaskResult::Failed {
            code: "EvaluationFailed".to_owned(),
            message: "attribute evaluation failed".to_owned(),
        }),
        ..FakeNix::default()
    };
    let (status, stdout, _) = invoke(request(), &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let result: Value = serde_json::from_slice(&stdout).expect("evaluation failure");
    assert_eq!(result["failures"][0]["code"], "EvaluationFailed");

    let (root2, cache2) = fixture(true);
    let blocked_cache = root2.join("not-a-directory");
    fs::write(&blocked_cache, "file").expect("cache collision");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let backend = FakeNix::default();
    let status = run_at_with(
        request(),
        CheckContext {
            root: &root2,
            json_mode: true,
            backend: &backend,
            interrupt: InterruptFlag::for_test(),
            cache_base: Some(&blocked_cache),
            terminal: crate::output::TerminalFacts::default(),
        },
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(status, std::process::ExitCode::from(2));
    let error: Value = serde_json::from_slice(&stdout).expect("operational error");
    assert_eq!(error["error"]["code"], "OperationalError");
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
    let _ = fs::remove_dir_all(root2);
    let _ = fs::remove_dir_all(cache2);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-005"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-007"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-008"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-010"))]
fn fail_fast_retains_started_results_and_does_not_admit_queued_work() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["broken".to_owned(), "queued".to_owned()],
        unique_derivations: true,
        realization_result: Some(NixTaskResult::Failed {
            code: "BuildFailed".to_owned(),
            message: "failed build".to_owned(),
        }),
        ..FakeNix::default()
    };
    let mut args = request();
    args.fail_fast = true;
    args.jobs = Some(1);
    let (status, stdout, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let result: Value = serde_json::from_slice(&stdout).expect("summary");
    assert!(result["counts"]["not_run"].as_u64().unwrap_or(0) > 0);
    assert_eq!(backend.realizations.load(Ordering::SeqCst), 1);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-OUTPUT-007"))]
fn failure_ids_are_stable_when_task_completion_order_changes() {
    let make_failures = || {
        vec![
            FailureDraft {
                check: "nix:x86_64-linux:z".to_owned(),
                code: "BuildFailed".to_owned(),
                subject: None,
                location: None,
                message: "z failed".to_owned(),
                notes: Vec::new(),
                log: None,
                nix_log: None,
                focus_tail: true,
                occurrence: 0,
            },
            FailureDraft {
                check: "nix:x86_64-linux:a".to_owned(),
                code: "BuildFailed".to_owned(),
                subject: None,
                location: None,
                message: "a failed".to_owned(),
                notes: Vec::new(),
                log: None,
                nix_log: None,
                focus_tail: true,
                occurrence: 1,
            },
        ]
    };
    let forward = assign_failure_ids(make_failures());
    let reverse = assign_failure_ids(make_failures().into_iter().rev().collect());
    assert_eq!(forward, reverse);
    assert_eq!(forward[0].check, "nix:x86_64-linux:a");
    assert_eq!(forward[0].id, "f1");
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-016"))]
fn failure_to_finalize_a_run_never_returns_success() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["check".to_owned()],
        force_manifest_failure: true,
        ..FakeNix::default()
    };
    let (status, stdout, _) = invoke(request(), &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::from(2));
    let error: Value = serde_json::from_slice(&stdout).expect("finalization error");
    assert_eq!(error["error"]["code"], "OperationalError");
    assert!(error.get("run").and_then(Value::as_str).is_some());
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-001"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-002"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-003"))]
fn independent_nix_derivations_realize_concurrently_within_the_job_limit() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["first".to_owned(), "second".to_owned(), "third".to_owned()],
        unique_derivations: true,
        ..FakeNix::default()
    };
    let mut args = request();
    args.selectors = vec!["nix:*".to_owned()];
    args.jobs = Some(2);
    let (status, _, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    assert_eq!(backend.realizations.load(Ordering::SeqCst), 3);
    assert_eq!(backend.maximum_active.load(Ordering::SeqCst), 2);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-012"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-013"))]
fn interrupted_runs_retain_completed_outcomes_and_return_130() {
    let (root, cache) = fixture(true);
    let interrupt = InterruptFlag::for_test();
    let backend = FakeNix {
        names: vec!["interrupted".to_owned()],
        interrupt_on_evaluation: Some(interrupt.clone()),
        ..FakeNix::default()
    };
    let (status, stdout, _) =
        invoke_with_interrupt(request(), &root, &cache, &backend, true, interrupt);
    assert_eq!(status, std::process::ExitCode::from(130));
    let result: Value = serde_json::from_slice(&stdout).expect("interrupted summary");
    assert_eq!(result["status"], "interrupted");
    let store =
        crate::check_command::store::RunStore::open(&root, Some(&cache)).expect("retained store");
    let retained = store.latest().expect("interrupted run").0;
    assert_eq!(retained.status, RunStatus::Interrupted);
    assert!(
        retained
            .outcomes
            .iter()
            .any(|outcome| outcome.id == "static:structure" && outcome.outcome == Outcome::Passed)
    );
    assert!(
        retained
            .outcomes
            .iter()
            .any(|outcome| outcome.id == "nix:x86_64-linux:interrupted"
                && outcome.outcome == Outcome::Canceled)
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-006"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-009"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-005"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-009"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-006"))]
fn prerequisite_failures_block_dependents_without_suppressing_nix_checks() {
    let (root, cache) = fixture(true);
    fs::remove_file(root.join(".bloomery/specs/CLI/CHECK/README.md"))
        .expect("remove the static prerequisite");
    let backend = FakeNix {
        names: vec!["independent".to_owned()],
        ..FakeNix::default()
    };
    let (status, stdout, _) = invoke(request(), &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let result: Value = serde_json::from_slice(&stdout).expect("summary JSON");
    assert_eq!(result["counts"]["failed"], 1);
    assert_eq!(result["counts"]["blocked"], 1);
    assert_eq!(result["counts"]["passed"], 2);
    assert_eq!(backend.realizations.load(Ordering::SeqCst), 1);
    assert_eq!(
        backend.systems.lock().unwrap().as_slice(),
        &["x86_64-linux"]
    );
    assert_eq!(result["failures"].as_array().unwrap().len(), 1);
    let store =
        crate::check_command::store::RunStore::open(&root, Some(&cache)).expect("retained store");
    let retained = store.latest().expect("retained run").0;
    let blocked = retained
        .outcomes
        .iter()
        .find(|outcome| outcome.id == "static:traceability")
        .expect("blocked traceability outcome");
    assert_eq!(blocked.blocked_by.as_deref(), Some("f1"));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-RUN-015"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-OUTPUT-011"))]
fn successful_exit_requires_every_selected_check_to_pass() {
    let (root, cache) = fixture(true);
    let backend = FakeNix::default();
    let (status, _, _) = invoke(request(), &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-OUTPUT-001"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-OUTPUT-002"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-OUTPUT-003"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-OUTPUT-004"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-OUTPUT-005"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-OUTPUT-006"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-OUTPUT-007"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-OUTPUT-008"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-OUTPUT-009"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-OUTPUT-010"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-OUTPUT-011"))]
fn summaries_agree_between_human_and_json_and_do_not_stream_logs() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["failure".to_owned()],
        realization_result: Some(NixTaskResult::Failed {
            code: "BuildFailed".to_owned(),
            message: "diagnostic is longer than the summary".to_owned(),
        }),
        ..FakeNix::default()
    };
    let (json_status, json_bytes, _) = invoke(request(), &root, &cache, &backend, true);
    let (text_status, text_bytes, _) = invoke(request(), &root, &cache, &backend, false);
    assert_eq!(json_status, text_status);
    assert_eq!(json_status, std::process::ExitCode::FAILURE);
    let json: Value = serde_json::from_slice(&json_bytes).expect("compact JSON summary");
    let text = String::from_utf8(text_bytes).expect("human summary");
    assert!(text.contains("failed"));
    assert!(text.contains("1 failure record"));
    assert!(text.contains("f1"));
    assert!(!text.contains("build log output"));
    assert_eq!(json["counts"]["failed"], 1);
    assert!(json["failures"][0].get("message").is_none());
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-001"))]
fn every_execution_shape_formats_once_before_any_check() {
    for selectors in [
        Vec::new(),
        vec!["nix:*".to_owned()],
        vec!["static:*".to_owned()],
    ] {
        let (root, cache) = fixture(true);
        let backend = FakeNix {
            names: vec!["one".to_owned(), "two".to_owned()],
            ..FakeNix::default()
        };
        let mut args = request();
        args.selectors = selectors;
        let (status, _, _) = invoke(args, &root, &cache, &backend, true);
        assert_eq!(status, std::process::ExitCode::SUCCESS);
        assert_eq!(backend.formatter_runs.load(Ordering::SeqCst), 1);
        let events = backend.events.lock().unwrap().clone();
        assert_eq!(events.first().map(String::as_str), Some("format"));
        assert!(
            events[1..]
                .iter()
                .all(|event| event.starts_with("realize:"))
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(cache);
    }
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-002"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-006"))]
fn formatter_failure_stops_all_check_work_regardless_of_fail_fast() {
    for fail_fast in [false, true] {
        let (root, cache) = fixture(true);
        let backend = FakeNix {
            names: vec!["one".to_owned(), "two".to_owned()],
            unique_derivations: true,
            formatter_result: Some(NixTaskResult::Failed {
                code: "NixFormatFailed".to_owned(),
                message: "nix fmt failed with exit code 1".to_owned(),
            }),
            ..FakeNix::default()
        };
        let mut args = request();
        args.fail_fast = fail_fast;
        args.jobs = Some(1);
        let (status, stdout, _) = invoke(args, &root, &cache, &backend, true);
        assert_eq!(status, std::process::ExitCode::FAILURE);
        assert_eq!(backend.formatter_runs.load(Ordering::SeqCst), 1);
        assert_eq!(backend.realizations.load(Ordering::SeqCst), 0);
        let result: Value = serde_json::from_slice(&stdout).expect("summary");
        assert_eq!(result["counts"]["failed"], 1);
        assert!(result["counts"]["not_run"].as_u64().unwrap_or(0) >= 2);
        assert_eq!(result["failures"][0]["check"], "format:workspace");
        assert_eq!(result["failures"][0]["code"], "NixFormatFailed");
        let store = RunStore::open(&root, Some(&cache)).expect("retained store");
        let record = store.latest().expect("retained run").0;
        assert!(record.outcomes.iter().any(|outcome| {
            outcome.id == "format:workspace" && outcome.outcome == Outcome::Failed
        }));
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(cache);
    }
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-003"))]
fn selected_checks_run_against_the_formatted_workspace() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        formatter_removals: vec![".bloomery/specs/CLI/CHECK/README.md".to_owned()],
        ..FakeNix::default()
    };
    let mut args = request();
    args.selectors = vec!["static:*".to_owned()];
    let (status, stdout, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let result: Value = serde_json::from_slice(&stdout).expect("summary");
    assert_eq!(result["failures"][0]["code"], "MissingDocument");
    assert!(!root.join(".bloomery/specs/CLI/CHECK/README.md").exists());
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-004"))]
fn partial_formatter_edits_persist_after_failure() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        formatter_edits: vec![("partial-format.txt".to_owned(), b"partial".to_vec())],
        formatter_result: Some(NixTaskResult::Failed {
            code: "NixFormatFailed".to_owned(),
            message: "nix fmt failed".to_owned(),
        }),
        ..FakeNix::default()
    };
    let (status, _, _) = invoke(request(), &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    assert_eq!(
        fs::read(root.join("partial-format.txt")).expect("partial formatter edit"),
        b"partial"
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-005"))]
fn read_only_check_operations_do_not_format() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["one".to_owned()],
        ..FakeNix::default()
    };
    let operations = [
        CheckOperation::List(ListArgs::default()),
        CheckOperation::Failures(FailureArgs {
            run: None,
            offset: 0,
            limit: 20,
        }),
        CheckOperation::Details(DetailsArgs {
            failure: "f1".to_owned(),
            run: None,
            offset: None,
            limit: 20,
        }),
    ];
    for operation in operations {
        let mut args = request();
        args.operation = Some(operation);
        let _ = invoke(args, &root, &cache, &backend, true);
        assert_eq!(backend.formatter_runs.load(Ordering::SeqCst), 0);
    }
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-007"))]
fn formatter_output_is_captured_in_a_run_local_log_and_not_streamed() {
    let (root, cache) = fixture(true);
    let secret = "formatter secret output line";
    let backend = FakeNix {
        names: vec!["one".to_owned()],
        formatter_log_contents: Some(format!("{secret}\n").into_bytes()),
        ..FakeNix::default()
    };
    let (status, stdout, stderr) = invoke(request(), &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    assert!(!String::from_utf8_lossy(&stdout).contains(secret));
    assert!(!String::from_utf8_lossy(&stderr).contains(secret));
    let store = RunStore::open(&root, Some(&cache)).expect("retained store");
    let (record, directory) = store.latest().expect("retained run");
    let format = record
        .outcomes
        .iter()
        .find(|outcome| outcome.id == "format:workspace")
        .expect("retained format outcome");
    assert_eq!(format.logs.len(), 1);
    let path = crate::check_command::store::resolve_log_path(&directory, &format.logs[0])
        .expect("formatter log path");
    assert!(fs::read_to_string(path).unwrap().contains(secret));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

fn git_is_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn run_git(root: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .status()
        .expect("start git");
    assert!(status.success(), "git {arguments:?} failed");
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-DETAIL-023"))]
fn git_dirty_state_is_sampled_after_formatter_edits() {
    if !git_is_available() {
        eprintln!("skipping post-format git-state test: git is unavailable");
        return;
    }
    let (root, cache) = fixture(true);
    run_git(&root, &["init", "-q"]);
    run_git(&root, &["add", "-A"]);
    run_git(
        &root,
        &[
            "-c",
            "user.email=bloomery@example.invalid",
            "-c",
            "user.name=Bloomery",
            "commit",
            "-q",
            "-m",
            "fixture",
        ],
    );
    let backend = FakeNix {
        names: vec!["one".to_owned()],
        formatter_edits: vec![("formatted-source.rs".to_owned(), b"// formatted\n".to_vec())],
        ..FakeNix::default()
    };
    let (status, _, _) = invoke(request(), &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let store = RunStore::open(&root, Some(&cache)).expect("retained store");
    let record = store.latest().expect("retained run").0;
    assert_eq!(record.source_dirty, Some(true));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}
