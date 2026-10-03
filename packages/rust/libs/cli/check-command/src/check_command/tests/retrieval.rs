use crate::check_command::catalog::NixTaskResult;
use crate::check_command::command::{CheckOperation, DetailsArgs, FailureArgs, ListArgs};
use crate::check_command::test_support::{FakeNix, fixture, invoke, request};
use bloomery_test_macros::bloomery;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::sync::Mutex;
use std::sync::atomic::Ordering;

#[test]
#[bloomery("CLI-CHECK-SELECT-021")]
fn oversized_catalog_ids_return_an_error_instead_of_a_stalled_page() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["x".repeat(9000)],
        ..FakeNix::default()
    };
    for json_mode in [true, false] {
        let mut args = request();
        args.operation = Some(CheckOperation::List(ListArgs {
            selectors: vec!["nix:*".to_owned()],
            systems: Vec::new(),
            offset: 0,
            limit: 20,
        }));
        let (status, output, errors) = invoke(args, &root, &cache, &backend, json_mode);
        assert_eq!(status, std::process::ExitCode::from(2));
        assert!(output.len() + errors.len() <= 8192);
        if json_mode {
            let page: Value = serde_json::from_slice(&output).unwrap();
            assert_eq!(page["error"]["code"], "OperationalError");
        } else {
            assert!(String::from_utf8_lossy(&errors).contains("cannot fit a bounded page"));
        }
    }
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[bloomery("CLI-CHECK-DETAIL-027")]
#[bloomery("CLI-CHECK-DETAIL-028")]
#[bloomery("CLI-CHECK-DETAIL-008")]
fn concurrent_alias_retrieval_freezes_one_shared_derivation_log_snapshot() {
    let (root, cache) = fixture(true);
    let store_path = "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-failed-check.drv";
    let backend = FakeNix {
        names: vec!["alias-a".to_owned(), "alias-b".to_owned()],
        realization_result: Some(NixTaskResult::Failed {
            code: "NixCheckFailed".to_owned(),
            message: "failed".to_owned(),
        }),
        log_contents: Some(format!("For full logs, run: nix log {store_path}\n").into_bytes()),
        nix_logs: Mutex::new(BTreeMap::from([(
            store_path.to_owned(),
            Ok((0..80)
                .map(|i| format!("derivation line {i}\n"))
                .collect::<String>()
                .into_bytes()),
        )])),
        ..FakeNix::default()
    };
    let (_, summary, _) = invoke(request(), &root, &cache, &backend, true);
    let summary: Value = serde_json::from_slice(&summary).unwrap();
    let run = summary["run"].as_str().unwrap().to_owned();
    let (_, directory) = crate::check_command::store::RunStore::open(&root, Some(&cache))
        .unwrap()
        .load(&run)
        .unwrap();
    let manifest = fs::read(directory.join("manifest.json")).unwrap();
    let details = |failure: &str| {
        let mut args = request();
        args.operation = Some(CheckOperation::Details(DetailsArgs {
            failure: failure.to_owned(),
            run: Some(run.clone()),
            offset: Some(30),
            limit: 5,
        }));
        let (status, output, _) = invoke(args, &root, &cache, &backend, true);
        assert_eq!(status, std::process::ExitCode::SUCCESS);
        serde_json::from_slice::<Value>(&output).unwrap()
    };
    let pages = std::thread::scope(|scope| {
        let a = scope.spawn(|| details("f1"));
        let b = scope.spawn(|| details("f2"));
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_eq!(backend.nix_log_requests.load(Ordering::SeqCst), 1);
    assert_eq!(pages.0["records"], pages.1["records"]);
    assert_eq!(pages.0["total"], 82);
    backend
        .nix_logs
        .lock()
        .unwrap()
        .insert(store_path.to_owned(), Err("log disappeared".to_owned()));
    assert_eq!(details("f1"), pages.0);
    assert_eq!(backend.nix_log_requests.load(Ordering::SeqCst), 1);
    assert_eq!(fs::read(directory.join("manifest.json")).unwrap(), manifest);
    assert_eq!(
        fs::read_dir(directory.join("derivation-logs"))
            .unwrap()
            .filter(|entry| entry
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|ext| ext == "json"))
            .count(),
        1
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[bloomery("CLI-CHECK-DETAIL-008")]
#[bloomery("CLI-CHECK-DETAIL-009")]
#[bloomery("CLI-CHECK-DETAIL-011")]
#[bloomery("CLI-CHECK-DETAIL-012")]
#[bloomery("CLI-CHECK-DETAIL-018")]
fn diagnostics_keep_full_messages_and_default_details_start_at_the_failure() {
    let (root, cache) = fixture(false);
    let requirement = root.join(".bloomery/specs/CLI/CHECK/requirements/CONTRACT.toml");
    let contents = fs::read_to_string(&requirement).expect("fixture requirement");
    fs::write(
        &requirement,
        contents.replace("manual = true", "manual = false"),
    )
    .expect("make requirement automated");
    let backend = FakeNix::default();
    let (status, summary, _) = invoke(request(), &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let summary: Value = serde_json::from_slice(&summary).expect("summary");
    let run = summary["run"].as_str().expect("run ID").to_owned();
    let failure = summary["failures"][0]["id"].as_str().unwrap().to_owned();

    let mut args = request();
    args.operation = Some(CheckOperation::Details(DetailsArgs {
        failure,
        run: Some(run),
        offset: None,
        limit: 40,
    }));
    let (status, output, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let details: Value = serde_json::from_slice(&output).expect("structured details");
    assert_eq!(details["offset"], 0);
    assert_eq!(details["code"], "MissingAutomatedTest");
    assert!(
        details["location"]["path"]
            .as_str()
            .unwrap()
            .contains("requirements/CONTRACT.toml")
    );
    let displayed = details["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(displayed.contains("MissingAutomatedTest"));
    assert!(displayed.contains("Owner:"));
    let store =
        crate::check_command::store::RunStore::open(&root, Some(&cache)).expect("retained store");
    let retained = store.latest().expect("retained run").0;
    assert!(!retained.failures[0].message.is_empty());
    assert!(!retained.failures[0].notes.is_empty());
    assert!(retained.failures[0].location.is_some());
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[bloomery("CLI-CHECK-SELECT-007")]
#[bloomery("CLI-CHECK-SELECT-008")]
#[bloomery("CLI-INTERFACE-COMMANDS-006")]
#[bloomery("CLI-CHECK-DETAIL-007")]
fn listing_discovers_and_filters_ids_without_executing_builds() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["first".to_owned(), "second".to_owned()],
        ..FakeNix::default()
    };
    let mut args = request();
    args.operation = Some(CheckOperation::List(ListArgs {
        selectors: vec!["nix:*:first".to_owned()],
        systems: Vec::new(),
        offset: 0,
        limit: 20,
    }));
    let (status, stdout, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let result: Value = serde_json::from_slice(&stdout).expect("list JSON");
    assert_eq!(result["checks"].as_array().unwrap().len(), 1);
    assert_eq!(backend.evaluations.load(Ordering::SeqCst), 0);
    assert_eq!(backend.realizations.load(Ordering::SeqCst), 0);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[bloomery("CLI-CHECK-DETAIL-004")]
#[bloomery("CLI-CHECK-DETAIL-005")]
#[bloomery("CLI-CHECK-DETAIL-006")]
#[bloomery("CLI-CHECK-DETAIL-014")]
#[bloomery("CLI-CHECK-DETAIL-017")]
#[bloomery("CLI-CHECK-OUTPUT-010")]
#[bloomery("CLI-INTERFACE-COMMANDS-007")]
fn retrieval_pages_failures_and_details_without_reexecuting_work() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        names: vec!["broken".to_owned()],
        realization_result: Some(NixTaskResult::Failed {
            code: "BuildFailed".to_owned(),
            message: "build failed; see retained log".to_owned(),
        }),
        ..FakeNix::default()
    };
    let (status, _, _) = invoke(request(), &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let evaluations = backend.evaluations.load(Ordering::SeqCst);
    let builds = backend.realizations.load(Ordering::SeqCst);

    let mut args = request();
    args.operation = Some(CheckOperation::Failures(FailureArgs {
        run: None,
        offset: 0,
        limit: 20,
    }));
    let (status, failures, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let failures: Value = serde_json::from_slice(&failures).expect("failure page");
    let id = failures["failures"][0]["id"].as_str().unwrap().to_owned();

    let mut args = request();
    args.operation = Some(CheckOperation::Details(DetailsArgs {
        failure: id,
        run: None,
        offset: None,
        limit: 40,
    }));
    let (status, details, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let details: Value = serde_json::from_slice(&details).expect("details page");
    assert_eq!(details["code"], "BuildFailed");
    assert!(details["records"].as_array().unwrap().iter().any(|line| {
        line.as_str()
            .is_some_and(|line| line.contains("build log output"))
    }));
    assert_eq!(backend.evaluations.load(Ordering::SeqCst), evaluations);
    assert_eq!(backend.realizations.load(Ordering::SeqCst), builds);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[bloomery("CLI-CHECK-DETAIL-025")]
fn detail_retrieval_loads_a_retained_nix_log_on_demand() {
    let (root, cache) = fixture(true);
    let store_path = "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-failed-check.drv";
    let backend = FakeNix {
        names: vec!["broken".to_owned()],
        realization_result: Some(NixTaskResult::Failed {
            code: "NixCheckFailed".to_owned(),
            message: "build failed".to_owned(),
        }),
        log_contents: Some(format!("For full logs, run: nix log {store_path}\n").into_bytes()),
        nix_logs: Mutex::new(BTreeMap::from([(
            store_path.to_owned(),
            Ok(b"full derivation log: missing requirement\n".to_vec()),
        )])),
        ..FakeNix::default()
    };
    let (status, summary, _) = invoke(request(), &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    assert_eq!(backend.nix_log_requests.load(Ordering::SeqCst), 0);
    let summary: Value = serde_json::from_slice(&summary).expect("check summary");
    let run = summary["run"].as_str().expect("run ID").to_owned();
    let failure = summary["failures"][0]["id"]
        .as_str()
        .expect("failure ID")
        .to_owned();

    let mut args = request();
    args.operation = Some(CheckOperation::Details(DetailsArgs {
        failure,
        run: Some(run),
        offset: Some(0),
        limit: 40,
    }));
    let (status, details, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    assert_eq!(backend.nix_log_requests.load(Ordering::SeqCst), 1);
    let details: Value = serde_json::from_slice(&details).expect("details page");
    let records = details["records"]
        .as_array()
        .expect("detail records")
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(records.contains("full derivation log: missing requirement"));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[bloomery("CLI-CHECK-DETAIL-026")]
#[bloomery("CLI-CHECK-DETAIL-027")]
fn an_unavailable_nix_log_preserves_captured_failure_details() {
    let (root, cache) = fixture(true);
    let store_path = "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-failed-check.drv";
    let backend = FakeNix {
        names: vec!["broken".to_owned()],
        realization_result: Some(NixTaskResult::Failed {
            code: "NixCheckFailed".to_owned(),
            message: "build failed".to_owned(),
        }),
        log_contents: Some(
            format!("For full logs, run: nix log {store_path}\noriginal build output\n")
                .into_bytes(),
        ),
        nix_logs: Mutex::new(BTreeMap::from([(
            store_path.to_owned(),
            Err("store log is unavailable".to_owned()),
        )])),
        ..FakeNix::default()
    };
    let (status, summary, _) = invoke(request(), &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let summary: Value = serde_json::from_slice(&summary).expect("check summary");
    let mut args = request();
    args.operation = Some(CheckOperation::Details(DetailsArgs {
        failure: summary["failures"][0]["id"]
            .as_str()
            .expect("failure ID")
            .to_owned(),
        run: Some(summary["run"].as_str().expect("run ID").to_owned()),
        offset: Some(0),
        limit: 40,
    }));
    let (status, details, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let details: Value = serde_json::from_slice(&details).expect("details page");
    let records = details["records"]
        .as_array()
        .expect("detail records")
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(records.contains("original build output"));
    assert!(records.contains("Nix derivation log unavailable: store log is unavailable"));
    backend
        .nix_logs
        .lock()
        .unwrap()
        .insert(store_path.to_owned(), Ok(b"newly available log\n".to_vec()));
    let mut args = request();
    args.operation = Some(CheckOperation::Details(DetailsArgs {
        failure: summary["failures"][0]["id"].as_str().unwrap().to_owned(),
        run: Some(summary["run"].as_str().unwrap().to_owned()),
        offset: Some(0),
        limit: 40,
    }));
    let (_, repeated, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(serde_json::from_slice::<Value>(&repeated).unwrap(), details);
    assert_eq!(backend.nix_log_requests.load(Ordering::SeqCst), 1);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[bloomery("CLI-CHECK-DETAIL-015")]
#[bloomery("CLI-CHECK-DETAIL-016")]
#[bloomery("CLI-CHECK-DETAIL-020")]
fn retrieval_rejects_missing_runs_and_preserves_only_safe_display_records() {
    let (root, cache) = fixture(false);
    let mut args = request();
    args.operation = Some(CheckOperation::Details(DetailsArgs {
        failure: "f1".to_owned(),
        run: Some("missing".to_owned()),
        offset: Some(0),
        limit: 40,
    }));
    let (status, stdout, _) = invoke(args, &root, &cache, &FakeNix::default(), true);
    assert_eq!(status, std::process::ExitCode::from(2));
    let error: Value = serde_json::from_slice(&stdout).expect("retrieval error");
    assert_eq!(error["error"]["code"], "RetrievalError");
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}
