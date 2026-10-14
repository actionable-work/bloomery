use crate::check_command::command::{CheckOperation, HealthArgs, HealthUpdateArgs};
use crate::check_command::model::Outcome;
use crate::check_command::store::RunStore;
use crate::check_command::test_support::{FakeNix, fixture_without_specs, invoke, request};
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
        "[specs]\nenable = false\n\n[checks.codeHealth]\nenable = true\nminCrateScore = 0\nminWorkspaceScore = 0\ncyclomaticLimit = 2\n",
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
        "[specs]\nenable = false\n\n[checks.codeHealth]\nenable = true\nminCrateScore = 0\nminWorkspaceScore = 0\ncyclomaticLimit = 9\n",
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
