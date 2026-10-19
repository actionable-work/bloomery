use crate::check_command::command::{CheckOperation, HealthArgs, HealthUpdateArgs};
use crate::check_command::model::Outcome;
use crate::check_command::store::RunStore;
use crate::check_command::test_support::{
    FakeNix, fixture_without_specs, invoke, invoke_with_interrupt_and_mode, invoke_with_mode,
    request,
};
use bloomery_check::{HealthSourceCrate, HealthTarget};
use serde_json::Value;
use std::fs;
use std::sync::atomic::Ordering;

fn health_fixture() -> (std::path::PathBuf, std::path::PathBuf, FakeNix) {
    let config = "[specs]\nenable = false\n\n[checks.codeHealth]\nenable = true\nminCrateScore = 0\nminWorkspaceScore = 0\n";
    let (root, cache) = fixture_without_specs(true, config);
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/demo\"]\nresolver = \"3\"\n",
    )
    .expect("workspace manifest");
    fs::create_dir_all(root.join("crates/demo/src")).expect("crate source directory");
    fs::write(
        root.join("crates/demo/Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("crate manifest");
    fs::write(
        root.join("crates/demo/src/lib.rs"),
        "pub fn useful(value: bool) -> usize { if value { 1 } else { 0 } }\n",
    )
    .expect("crate source");
    let backend = FakeNix {
        health_crates: vec![HealthSourceCrate {
            name: "demo".to_owned(),
            manifest_path: "crates/demo/Cargo.toml".to_owned(),
            targets: vec![HealthTarget {
                kind: "library".to_owned(),
                path: "src/lib.rs".to_owned(),
            }],
        }],
        ..FakeNix::default()
    };
    (root, cache, backend)
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-002"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-015"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-017"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-021"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-018"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-003"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-032"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-033"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-036"))]
#[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-009"))]
#[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-013"))]
fn health_runs_without_specs_and_retains_measurements_when_baseline_is_missing() {
    let (root, cache, backend) = health_fixture();
    let mut args = request();
    args.selectors = vec!["static:code-health".to_owned()];
    args.jobs = Some(1);
    let (status, stdout, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let summary: Value = serde_json::from_slice(&stdout).expect("run summary");
    let run = summary["result"]["run"].as_str().expect("run ID");
    let store = RunStore::open(&root, Some(&cache)).expect("run store");
    let (record, directory) = store.load(run).expect("retained run");
    assert_eq!(
        record
            .outcomes
            .iter()
            .find(|item| item.id == "static:code-health")
            .unwrap()
            .outcome,
        Outcome::Failed
    );
    assert!(record.health_baseline_refreshed);
    assert_eq!(
        record.health_baseline_path.as_deref(),
        Some(".bloomery/code-health-summary.json")
    );
    let baseline: Value = serde_json::from_slice(
        &fs::read(root.join(".bloomery/code-health-summary.json")).expect("seeded baseline"),
    )
    .expect("seeded baseline JSON");
    assert_eq!(baseline["schema_version"], 3);
    assert!(
        record
            .notices
            .iter()
            .any(|notice| notice.code == "HealthReportRetained")
    );
    let report: Value = serde_json::from_slice(
        &fs::read(directory.join("health-report.json")).expect("report artifact"),
    )
    .expect("health report JSON");
    assert_eq!(
        report["crates"][0]["manifest_path"],
        "crates/demo/Cargo.toml"
    );
    assert_eq!(
        report["crates"][0]["callables"].as_array().unwrap().len(),
        1
    );

    let mut retrieval = request();
    retrieval.operation = Some(CheckOperation::Health(HealthArgs {
        run: Some(run.to_owned()),
        offset: 0,
        limit: 20,
    }));
    let (status, output, _) = invoke(retrieval, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let page: Value = serde_json::from_slice(&output).expect("health page");
    assert_eq!(
        page["result"]["crates"][0]["manifest_path"],
        "crates/demo/Cargo.toml"
    );
    assert_eq!(backend.formatter_runs.load(Ordering::SeqCst), 1);

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-035"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-022"))]
fn no_write_health_preserves_the_baseline_and_can_be_explicitly_published() {
    let (root, cache, write_backend) = health_fixture();
    let mut write_args = request();
    write_args.selectors = vec!["static:code-health".to_owned()];
    let (_, first_output, _) = invoke(write_args, &root, &cache, &write_backend, true);
    let first: Value = serde_json::from_slice(&first_output).expect("write-mode summary");
    assert_eq!(first["result"]["mode"], "write");
    let baseline_path = root.join(".bloomery/code-health-summary.json");
    let before = fs::read(&baseline_path).expect("automatically refreshed baseline");

    let no_write_backend = FakeNix {
        names: vec!["core:treefmt-check".to_owned()],
        health_crates: write_backend.health_crates.clone(),
        ..FakeNix::default()
    };
    let mut no_write_args = request();
    no_write_args.selectors = vec!["static:code-health".to_owned()];
    let (status, output, _) =
        invoke_with_mode(no_write_args, &root, &cache, &no_write_backend, true, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let summary: Value = serde_json::from_slice(&output).expect("no-write summary");
    assert_eq!(summary["result"]["mode"], "no-write");
    assert_eq!(
        fs::read(&baseline_path).expect("unchanged baseline"),
        before
    );
    let store = RunStore::open(&root, Some(&cache)).expect("run store");
    let (record, _) = store.latest().expect("latest run");
    assert_eq!(record.mode, crate::check_command::model::RunMode::NoWrite);
    assert!(!record.health_baseline_refreshed);
    assert!(record.outcomes.iter().any(|outcome| {
        outcome.id == "nix:x86_64-linux:core:treefmt-check" && outcome.outcome == Outcome::Passed
    }));

    let mut update = request();
    update.operation = Some(CheckOperation::HealthUpdate(HealthUpdateArgs {
        run: Some(record.id),
    }));
    assert_eq!(
        invoke(update, &root, &cache, &no_write_backend, true).0,
        std::process::ExitCode::SUCCESS
    );
    assert_eq!(fs::read(&baseline_path).expect("explicit update"), before);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-033"))]
fn selected_health_refreshes_even_when_an_unrelated_nix_check_fails() {
    let (root, cache, health_backend) = health_fixture();
    let backend = FakeNix {
        names: vec!["broken".to_owned()],
        health_crates: health_backend.health_crates.clone(),
        realization_result: Some(crate::check_command::catalog::NixTaskResult::Failed {
            code: "NixCheckFailed".to_owned(),
            message: "independent Nix check failed".to_owned(),
        }),
        ..FakeNix::default()
    };
    let (status, output, _) = invoke(request(), &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let summary: Value = serde_json::from_slice(&output).expect("summary");
    let failure_codes = summary["result"]["failures"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|failure| failure["code"].as_str())
        .collect::<Vec<_>>();
    assert!(failure_codes.contains(&"NixCheckFailed"));
    let run = summary["result"]["run"].as_str().expect("run ID");
    let store = RunStore::open(&root, Some(&cache)).expect("run store");
    let record = store.load(run).expect("retained run").0;
    assert!(record.health_baseline_refreshed);
    assert!(root.join(".bloomery/code-health-summary.json").is_file());
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-034"))]
fn unselected_health_does_not_refresh_the_configured_baseline() {
    let (root, cache) = crate::check_command::test_support::fixture(true);
    fs::write(
        root.join(".bloomery/config.toml"),
        "version = 1\n[specs]\ndir = \"specs\"\n\n[checks.specifications]\nenable = true\n\n[checks.codeHealth]\nenable = true\nminCrateScore = 0\nminWorkspaceScore = 0\n",
    )
    .expect("enable health beside structure check");
    let backend = FakeNix::default();
    let baseline = root.join(".bloomery/code-health-summary.json");
    fs::write(&baseline, b"reviewed baseline\n").expect("previous baseline");
    let mut args = request();
    args.selectors = vec!["static:structure".to_owned()];
    let (status, _, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    assert_eq!(
        fs::read(&baseline).expect("unchanged baseline"),
        b"reviewed baseline\n"
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-039"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-028"))]
fn automatic_baseline_write_failure_retains_measurement_and_preserves_files() {
    let (root, cache, backend) = health_fixture();
    let baseline = root.join(".bloomery/code-health-summary.json");
    let previous = b"previous baseline contents\n";
    fs::write(&baseline, previous).expect("previous baseline");
    let collision = root.join(format!(
        ".bloomery/.code-health-summary.json.{}.tmp",
        std::process::id()
    ));
    fs::write(&collision, b"do not remove\n").expect("temporary-path collision");
    let mut args = request();
    args.selectors = vec!["static:code-health".to_owned()];
    let (status, output, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let summary: Value = serde_json::from_slice(&output).expect("summary");
    assert!(
        summary["result"]["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|failure| failure["code"] == "HealthBaselineWriteFailed")
    );
    assert_eq!(
        fs::read(&baseline).expect("previous baseline preserved"),
        previous
    );
    assert_eq!(
        fs::read(&collision).expect("foreign temp preserved"),
        b"do not remove\n"
    );
    let run = summary["result"]["run"].as_str().expect("run ID");
    let store = RunStore::open(&root, Some(&cache)).expect("run store");
    let (_, directory) = store.load(run).expect("retained run");
    assert!(directory.join("health-report.json").is_file());
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-037"))]
fn failed_formatting_and_early_interrupt_leave_health_baselines_unchanged() {
    let (root, cache, health_backend) = health_fixture();
    let baseline = root.join(".bloomery/code-health-summary.json");
    let previous = b"reviewed baseline\n";
    fs::write(&baseline, previous).expect("previous baseline");
    let failed_formatter = FakeNix {
        health_crates: health_backend.health_crates.clone(),
        formatter_result: Some(crate::check_command::catalog::NixTaskResult::Failed {
            code: "NixFormatFailed".to_owned(),
            message: "formatter failed".to_owned(),
        }),
        ..FakeNix::default()
    };
    let mut args = request();
    args.selectors = vec!["static:code-health".to_owned()];
    assert_eq!(
        invoke(args, &root, &cache, &failed_formatter, true).0,
        std::process::ExitCode::FAILURE
    );
    assert_eq!(
        fs::read(&baseline).expect("baseline after formatter failure"),
        previous
    );

    let interrupt = crate::check_command::interrupt::InterruptFlag::for_test();
    let interrupted_backend = FakeNix {
        health_crates: health_backend.health_crates,
        interrupt_on_health_source: Some(interrupt.clone()),
        ..FakeNix::default()
    };
    let mut args = request();
    args.selectors = vec!["static:code-health".to_owned()];
    let (status, _, _) = invoke_with_interrupt_and_mode(
        args,
        &root,
        &cache,
        &interrupted_backend,
        true,
        interrupt,
        false,
    );
    assert_eq!(status, std::process::ExitCode::from(130));
    assert_eq!(
        fs::read(&baseline).expect("baseline after interruption"),
        previous
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-038"))]
fn no_write_rejects_explicit_health_update_before_loading_the_run() {
    let (root, cache) = fixture_without_specs(
        true,
        "[specs]\nenable = false\n[checks.codeHealth]\nenable = false\n",
    );
    let baseline = root.join(".bloomery/code-health-summary.json");
    fs::write(&baseline, b"unchanged\n").expect("baseline");
    let mut args = request();
    args.operation = Some(CheckOperation::HealthUpdate(HealthUpdateArgs {
        run: Some("missing-run".to_owned()),
    }));
    let (status, output, _) =
        invoke_with_mode(args, &root, &cache, &FakeNix::default(), true, true);
    assert_eq!(status, std::process::ExitCode::from(2));
    let error: Value = serde_json::from_slice(&output).expect("usage error");
    assert_eq!(error["error"]["code"], "UsageError");
    assert!(!cache.exists());
    assert_eq!(
        fs::read(baseline).expect("unchanged baseline"),
        b"unchanged\n"
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-022"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-023"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-024"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-025"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-026"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-027"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-028"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-031"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-029"))]
fn health_update_exports_current_snapshot_and_rejects_stale_sources() {
    let (root, cache, backend) = health_fixture();
    let mut args = request();
    args.selectors = vec!["static:code-health".to_owned()];
    args.jobs = Some(1);
    let (_, stdout, _) = invoke(args, &root, &cache, &backend, true);
    let summary: Value = serde_json::from_slice(&stdout).expect("run summary");
    let run = summary["result"]["run"]
        .as_str()
        .expect("run ID")
        .to_owned();
    let mut update = request();
    update.operation = Some(CheckOperation::HealthUpdate(HealthUpdateArgs {
        run: Some(run.clone()),
    }));
    let (status, _, _) = invoke(update.clone(), &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let summary_file = root.join(".bloomery/code-health-summary.json");
    let before = fs::read(&summary_file).expect("exported summary baseline");
    assert!(before.ends_with(b"\n"));
    assert!(before.starts_with(b"{\n"));
    assert!(!root.join(".bloomery/code-health.json").exists());
    let summary: Value = serde_json::from_slice(&before).expect("PR summary baseline");
    assert_eq!(summary["schema_version"], 3);
    assert!(summary["workspace"]["overall_score"].is_number());
    assert!(summary["workspace"]["sloc"].is_number());
    assert!(summary["workspace"]["subscores"]["complexity"].is_number());
    assert!(summary["crates"][0]["overall_score"].is_number());
    assert!(summary["crates"][0]["subscores"]["duplication"].is_number());
    assert!(summary["crates"][0].get("callables").is_none());
    assert!(summary["crates"][0].get("sources").is_none());
    assert!(summary["input_digest"].is_string());

    let (status, _, _) = invoke(update.clone(), &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    assert_eq!(
        fs::read(&summary_file).expect("re-exported summary baseline"),
        before
    );

    fs::write(
        root.join("crates/demo/src/lib.rs"),
        "pub fn useful(value: bool) -> usize { if value { 2 } else { 0 } }\n",
    )
    .expect("change source");
    let (status, output, _) = invoke(update, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let error: Value = serde_json::from_slice(&output).expect("stale export result");
    assert_eq!(error["error"]["code"], "HealthExportStale");
    assert_eq!(
        fs::read(&summary_file).expect("unchanged summary baseline"),
        before
    );
    assert_eq!(backend.formatter_runs.load(Ordering::SeqCst), 1);

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-012"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-013"))]
fn health_regressions_compare_crate_and_workspace_scores_after_export() {
    let (root, cache, backend) = health_fixture();
    fs::write(
        root.join(".bloomery/config.toml"),
        "version = 1\n[specs]\nenable = false\n\n[checks.codeHealth]\nenable = true\nminCrateScore = 0\nminWorkspaceScore = 0\ncyclomaticLimit = 2\n",
    )
    .expect("health policy");
    let mut args = request();
    args.selectors = vec!["static:code-health".to_owned()];
    let (_, stdout, _) = invoke(args, &root, &cache, &backend, true);
    let summary: Value = serde_json::from_slice(&stdout).expect("initial summary");
    let run = summary["result"]["run"]
        .as_str()
        .expect("run ID")
        .to_owned();
    let mut update = request();
    update.operation = Some(CheckOperation::HealthUpdate(HealthUpdateArgs {
        run: Some(run),
    }));
    assert_eq!(
        invoke(update, &root, &cache, &backend, true).0,
        std::process::ExitCode::SUCCESS
    );

    fs::write(
        root.join("crates/demo/src/lib.rs"),
        "pub fn useful(value: bool) -> usize { if value || value { 1 } else { 0 } }\n",
    )
    .expect("regressed source");
    let mut args = request();
    args.selectors = vec!["static:code-health".to_owned()];
    let (status, stdout, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let summary: Value = serde_json::from_slice(&stdout).expect("regression summary");
    let run = summary["result"]["run"].as_str().expect("run ID");
    let store = RunStore::open(&root, Some(&cache)).expect("run store");
    let (_, directory) = store.load(run).expect("retained run");
    let report: Value = serde_json::from_slice(
        &fs::read(directory.join("health-report.json")).expect("health report"),
    )
    .expect("health report JSON");
    assert_eq!(report["baseline_status"], "compatible");
    assert!(report["workspace_delta_hundredths"].as_i64().unwrap() < 0);
    let failure_codes = summary["result"]["failures"]
        .as_array()
        .expect("failures")
        .iter()
        .filter_map(|failure| failure["code"].as_str())
        .collect::<Vec<_>>();
    assert!(failure_codes.contains(&"HealthCrateRegression"));
    assert!(failure_codes.contains(&"HealthWorkspaceRegression"));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-014"))]
fn health_reports_added_and_removed_crate_membership() {
    let (root, cache, original_backend) = health_fixture();
    let mut args = request();
    args.selectors = vec!["static:code-health".to_owned()];
    let (_, stdout, _) = invoke(args, &root, &cache, &original_backend, true);
    let summary: Value = serde_json::from_slice(&stdout).expect("initial summary");
    let run = summary["result"]["run"]
        .as_str()
        .expect("run ID")
        .to_owned();
    let mut update = request();
    update.operation = Some(CheckOperation::HealthUpdate(HealthUpdateArgs {
        run: Some(run),
    }));
    assert_eq!(
        invoke(update, &root, &cache, &original_backend, true).0,
        std::process::ExitCode::SUCCESS
    );

    fs::create_dir_all(root.join("crates/new/src")).expect("new source directory");
    fs::write(
        root.join("crates/new/Cargo.toml"),
        "[package]\nname = \"new\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("new manifest");
    fs::write(
        root.join("crates/new/src/lib.rs"),
        "pub fn new_crate() {}\n",
    )
    .expect("new source");
    let changed_backend = FakeNix {
        health_crates: vec![HealthSourceCrate {
            name: "new".to_owned(),
            manifest_path: "crates/new/Cargo.toml".to_owned(),
            targets: vec![HealthTarget {
                kind: "library".to_owned(),
                path: "src/lib.rs".to_owned(),
            }],
        }],
        ..FakeNix::default()
    };
    let mut args = request();
    args.selectors = vec!["static:code-health".to_owned()];
    let (_, stdout, _) = invoke(args, &root, &cache, &changed_backend, true);
    let summary: Value = serde_json::from_slice(&stdout).expect("membership summary");
    let run = summary["result"]["run"].as_str().expect("run ID");
    let store = RunStore::open(&root, Some(&cache)).expect("run store");
    let (_, directory) = store.load(run).expect("retained run");
    let report: Value = serde_json::from_slice(
        &fs::read(directory.join("health-report.json")).expect("health report"),
    )
    .expect("health report JSON");
    assert_eq!(
        report["added_crates"],
        serde_json::json!(["crates/new/Cargo.toml"])
    );
    assert_eq!(
        report["removed_crates"],
        serde_json::json!(["crates/demo/Cargo.toml"])
    );
    assert!(report["crate_comparisons"][0]["baseline_score_hundredths"].is_null());
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-016"))]
fn health_rejects_baselines_when_scoring_semantics_change() {
    let (root, cache, backend) = health_fixture();
    let mut args = request();
    args.selectors = vec!["static:code-health".to_owned()];
    let (_, stdout, _) = invoke(args, &root, &cache, &backend, true);
    let summary: Value = serde_json::from_slice(&stdout).expect("initial summary");
    let run = summary["result"]["run"]
        .as_str()
        .expect("run ID")
        .to_owned();
    let mut update = request();
    update.operation = Some(CheckOperation::HealthUpdate(HealthUpdateArgs {
        run: Some(run),
    }));
    assert_eq!(
        invoke(update, &root, &cache, &backend, true).0,
        std::process::ExitCode::SUCCESS
    );

    fs::write(
        root.join(".bloomery/config.toml"),
        "version = 1\n[specs]\nenable = false\n\n[checks.codeHealth]\nenable = true\nminCrateScore = 0\nminWorkspaceScore = 0\ncyclomaticLimit = 9\n",
    )
    .expect("changed scoring policy");
    let mut args = request();
    args.selectors = vec!["static:code-health".to_owned()];
    let (status, stdout, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let summary: Value = serde_json::from_slice(&stdout).expect("incompatible summary");
    let run = summary["result"]["run"].as_str().expect("run ID");
    let store = RunStore::open(&root, Some(&cache)).expect("run store");
    let (_, directory) = store.load(run).expect("retained run");
    let report: Value = serde_json::from_slice(
        &fs::read(directory.join("health-report.json")).expect("health report"),
    )
    .expect("health report JSON");
    assert_eq!(report["baseline_status"], "incompatible");
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-020"))]
fn health_retrieval_does_not_fall_back_when_latest_report_is_missing() {
    let (root, cache, backend) = health_fixture();
    for _ in 0..2 {
        let mut args = request();
        args.selectors = vec!["static:code-health".to_owned()];
        invoke(args, &root, &cache, &backend, true);
    }
    let store = RunStore::open(&root, Some(&cache)).expect("run store");
    let (latest, directory) = store.latest().expect("latest run");
    fs::remove_file(directory.join("health-report.json")).expect("remove latest report");
    let mut retrieval = request();
    retrieval.operation = Some(CheckOperation::Health(HealthArgs {
        run: None,
        offset: 0,
        limit: 20,
    }));
    let (status, output, _) = invoke(retrieval, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::from(2));
    assert!(String::from_utf8_lossy(&output).contains(&latest.id));
    assert!(String::from_utf8_lossy(&output).contains("no complete health report"));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}
