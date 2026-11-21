use crate::check_command::catalog::{
    CheckPlan, NIX_DISCOVER_EXPRESSION, NixBackend, NixCli, PlanRequest, SystemCatalog,
    plan_expression,
};
use crate::check_command::command::{CheckArgs, CheckOperation, DetailsArgs};
use crate::check_command::interrupt::{CancellationToken, InterruptFlag};
use crate::check_command::model::Outcome;
use crate::check_command::progress::ProgressSink;
use crate::check_command::store::RunStore;
use crate::check_command::test_support::subprocess::NixFixture;
use crate::check_command::test_support::{fixture, invoke, invoke_with_mode, request};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

const SHARED: &str = "/nix/store/shared.drv";
const OUTPUT: &str = "/nix/store/shared-out";

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_owned()).collect()
}

fn eval_args(system: &str, expression: String) -> Vec<String> {
    vec![
        "eval".into(),
        "--no-write-lock-file".into(),
        "--no-update-lock-file".into(),
        "--json".into(),
        format!(".#checks.{system}"),
        "--apply".into(),
        expression,
    ]
}

fn discovery_args(system: &str) -> Vec<String> {
    eval_args(system, NIX_DISCOVER_EXPRESSION.to_owned())
}

/// Restricted plan evaluation of the selected attribute names.
fn plan_args(system: &str, names: &[&str]) -> Vec<String> {
    let mut names = names
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    names.sort();
    eval_args(system, plan_expression(&names))
}

fn record_args(output: &str, system: &str) -> Vec<String> {
    vec![
        "derivation".into(),
        "show".into(),
        "--no-write-lock-file".into(),
        "--no-update-lock-file".into(),
        format!(".#{output}.{system}"),
    ]
}

/// A generic flake without Bloomery catalog records.
fn missing_record(script: &mut NixFixture, output: &str, system: &str) -> PathBuf {
    let call = script.expect(&record_args(output, system), "");
    fs::write(call.join("status"), "1").unwrap();
    fs::write(
        call.join("stderr"),
        format!("error: flake 'path:/workspace' does not provide attribute '{output}.{system}'\n"),
    )
    .unwrap();
    call
}

/// A shown catalog record whose environment field holds `value` as JSON.
fn record(script: &mut NixFixture, output: &str, field: &str, system: &str, value: &Value) {
    let shown = json!({"derivations": {
        format!("/nix/store/{output}.drv"): {"env": {field: value.to_string()}},
    }});
    script.expect(&record_args(output, system), &shown.to_string());
}

fn host() -> String {
    NixCli::with_executable("/missing/nix")
        .host_system(Path::new("."))
        .expect("host system")
}

fn metadata(entries: &[(&str, &str, &[&str])]) -> Value {
    let mut derivations = serde_json::Map::new();
    for (drv, output, inputs) in entries {
        let dependencies = inputs
            .iter()
            .map(|input| ((*input).to_owned(), json!(["out"])))
            .collect::<serde_json::Map<_, _>>();
        derivations.insert(
            (*drv).to_owned(),
            json!({
                "outputs": {"out": {"path": output}}, "inputDrvs": dependencies,
            }),
        );
    }
    json!({"derivations": derivations})
}

fn plan_calls(script: &mut NixFixture, names: &[(&str, &str)], metadata: Value) {
    let plans = names
        .iter()
        .map(|(name, drv)| ((*name).to_owned(), json!({"success": true, "drvPath": drv})))
        .collect::<serde_json::Map<_, _>>();
    let selected = names.iter().map(|(name, _)| *name).collect::<Vec<_>>();
    script.expect(
        &plan_args("x86_64-linux", &selected),
        &json!({"legacy": false, "plan": plans}).to_string(),
    );
    let mut args = strings(&["derivation", "show"]);
    let mut names = names.to_vec();
    names.sort_by_key(|(name, _)| *name);
    args.extend(names.iter().map(|(_, drv)| (*drv).to_owned()));
    script.expect(&args, &metadata.to_string());
}

fn discover_calls(script: &mut NixFixture, system: &str, names: &[&str]) {
    missing_record(script, "bloomeryCatalog", system);
    let mut names = names.to_vec();
    names.sort();
    script.expect(&discovery_args(system), &json!(names).to_string());
}

fn formatter(script: &mut NixFixture) -> PathBuf {
    script.expect(
        &strings(&["fmt", "--no-write-lock-file", "--no-update-lock-file"]),
        "formatter stdout\n",
    )
}

fn build(script: &mut NixFixture, drvs: &[&str], fail_fast: bool) -> PathBuf {
    let mut args = strings(&[
        "build",
        "--no-link",
        "--no-write-lock-file",
        "--no-update-lock-file",
    ]);
    if !fail_fast {
        args.push("--keep-going".into());
    }
    args.extend(drvs.iter().map(|drv| format!("{drv}^*")));
    script.expect(&args, "")
}

fn validity(script: &mut NixFixture, paths: &[(&str, bool)]) {
    let result = paths
        .iter()
        .map(|(path, valid)| {
            (
                (*path).to_owned(),
                if *valid { json!({}) } else { Value::Null },
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let call = script.expect(
        &strings(&["path-info", "--json", "--stdin"]),
        &json!(result).to_string(),
    );
    fs::write(
        call.join("stdin"),
        format!(
            "{}\n",
            paths
                .iter()
                .map(|(path, _)| *path)
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )
    .unwrap();
}

fn selected() -> CheckArgs {
    let mut args = request();
    args.systems = vec!["x86_64-linux".into()];
    args.selectors = vec!["nix:*".into()];
    args
}

fn latest(root: &Path, cache: &Path) -> crate::check_command::model::RunRecord {
    RunStore::open(root, Some(cache))
        .unwrap()
        .latest()
        .unwrap()
        .0
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-TESTING-002"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-TESTING-003"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-TESTING-006"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-006"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-007"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-008"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-009"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-016"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-019"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-020"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-032"))]
fn production_adapter_preserves_names_and_builds_shared_derivations_once() {
    let (root, cache) = fixture(true);
    fs::write(root.join("flake.lock"), "fixture lock\n").unwrap();
    let mut script = NixFixture::new(&root);
    let names = [
        "alias-a",
        "alias-b",
        "with spaces",
        "quote\"",
        "Unicode ☃",
        "line\nbreak",
        "tab\tname",
        "control-\u{1}",
        "percent-%0A-#",
    ];
    let mut sorted = names.to_vec();
    sorted.sort();
    discover_calls(&mut script, "x86_64-linux", &names);
    let backend = script.backend();
    assert_eq!(
        backend.catalog(&root, "x86_64-linux").unwrap(),
        SystemCatalog {
            attributes: sorted.iter().map(|name| (*name).to_owned()).collect(),
            recorded: None,
        }
    );
    discover_calls(&mut script, "x86_64-linux", &names);
    formatter(&mut script);
    plan_calls(
        &mut script,
        &names.map(|name| (name, SHARED)),
        metadata(&[(SHARED, OUTPUT, &[])]),
    );
    build(&mut script, &[SHARED], false);
    // Output validation queries every selected alias in one request.
    validity(&mut script, &vec![(OUTPUT, true); names.len()]);
    let (status, stdout, stderr) = invoke(selected(), &root, &cache, &backend, true);
    assert_eq!(
        status,
        ExitCode::SUCCESS,
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr)
    );
    let record = latest(&root, &cache);
    for name in names {
        assert!(
            record
                .outcomes
                .iter()
                .any(|outcome| outcome.id == format!("nix:x86_64-linux:{name}")
                    && outcome.outcome == Outcome::Passed)
        );
    }
    assert_eq!(
        fs::read_to_string(root.join("flake.lock")).unwrap(),
        "fixture lock\n"
    );
    assert!(!root.join("result").exists());
    script.verify().unwrap();
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-011"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-020"))]
fn production_adapter_distinguishes_missing_and_empty_systems() {
    let (root, cache) = fixture(true);
    let mut script = NixFixture::new(&root);
    missing_record(&mut script, "bloomeryCatalog", "aarch64-darwin");
    let missing = script.expect(&discovery_args("aarch64-darwin"), "");
    fs::write(missing.join("status"), "1").unwrap();
    fs::write(missing.join("stderr"), "missing checks output\n").unwrap();
    let mut args = request();
    args.systems = vec!["aarch64-darwin".into()];
    let (status, stdout, _) = invoke(args, &root, &cache, &script.backend(), true);
    assert_eq!(status, ExitCode::from(2));
    let error: Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(error["error"]["code"], "OperationalError");
    discover_calls(&mut script, "aarch64-linux", &[]);
    // The formatter comes from the host catalog, read separately when the
    // host system is not selected.
    discover_calls(&mut script, &host(), &[]);
    formatter(&mut script);
    let mut args = request();
    args.systems = vec!["aarch64-linux".into()];
    let (status, _, _) = invoke(args, &root, &cache, &script.backend(), true);
    assert_eq!(status, ExitCode::SUCCESS);
    assert!(
        latest(&root, &cache)
            .outcomes
            .iter()
            .any(|outcome| outcome.id == "static:structure" && outcome.outcome == Outcome::Passed)
    );
    script.verify().unwrap();
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-OUTPUT-017"))]
fn operational_catalog_error_reports_the_fatal_cause_after_notices() {
    let mut messages = Vec::new();

    for json_mode in [true, false] {
        let (root, cache) = fixture(true);
        let mut script = NixFixture::new(&root);
        discover_calls(&mut script, "x86_64-linux", &["core:unit-tests"]);
        formatter(&mut script);
        let failed_plan = script.expect(&plan_args("x86_64-linux", &["core:unit-tests"]), "");
        fs::write(failed_plan.join("status"), "1").unwrap();
        fs::write(
            failed_plan.join("stderr"),
            "trace: Obsolete option programs.ruff.enable was renamed\nwarning: compatibility notice\nerror: builder for '/nix/store/opt-app-pgo-training.drv' failed with exit code 139\n",
        )
        .unwrap();

        let backend = script.backend();
        let (status, stdout, stderr) = invoke(selected(), &root, &cache, &backend, json_mode);
        assert_eq!(status, ExitCode::from(2));

        let message = if json_mode {
            let error: Value = serde_json::from_slice(&stdout).expect("JSON operational error");
            error["error"]["message"]
                .as_str()
                .expect("error message")
                .to_owned()
        } else {
            let rendered = String::from_utf8_lossy(&stderr);
            let message_start = rendered
                .find("Nix check catalog evaluation failed")
                .unwrap_or_else(|| panic!("human error message missing from: {rendered}"));
            rendered[message_start..]
                .trim()
                .trim_end_matches("\u{1b}[0m")
                .to_owned()
        };

        assert!(message.contains("Nix check catalog evaluation failed"));
        assert!(message.contains(
            "builder for '/nix/store/opt-app-pgo-training.drv' failed with exit code 139"
        ));
        assert!(!message.contains("Obsolete option"));
        messages.push(message);
        script.verify().unwrap();
    }

    assert_eq!(messages[0], messages[1]);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-OUTPUT-017"))]
fn operational_nix_error_falls_back_to_status_and_non_notice_detail() {
    let (root, _) = fixture(true);
    let mut script = NixFixture::new(&root);
    missing_record(&mut script, "bloomeryCatalog", "x86_64-linux");
    let failed_discovery = script.expect(&discovery_args("x86_64-linux"), "");
    fs::write(failed_discovery.join("status"), "7").unwrap();
    fs::write(
        failed_discovery.join("stderr"),
        "trace: Obsolete option programs.ruff.enable was renamed\nwarning: compatibility notice\nevaluation context could not be loaded\n",
    )
    .unwrap();

    let backend = script.backend();
    let error = backend
        .catalog(&root, "x86_64-linux")
        .expect_err("failed Nix discovery");

    assert!(error.contains("Nix check discovery failed with exit code 7"));
    assert!(error.contains("evaluation context could not be loaded"));
    assert!(!error.contains("Obsolete option"));
    script.verify().unwrap();
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-008"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-010"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-011"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-031"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-008"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-OUTPUT-001"))]
fn production_no_write_realizes_the_native_treefmt_check_without_formatting() {
    let (root, cache) = fixture(true);
    let sentinel = root.join("no-write-sentinel.txt");
    fs::write(&sentinel, "unchanged\n").unwrap();
    let mut script = NixFixture::new(&root);
    discover_calls(
        &mut script,
        "x86_64-linux",
        &["core:treefmt-check", "unselected"],
    );
    plan_calls(
        &mut script,
        &[("core:treefmt-check", SHARED)],
        metadata(&[(SHARED, OUTPUT, &[])]),
    );
    build(&mut script, &[SHARED], false);
    validity(&mut script, &[(OUTPUT, true)]);
    let backend = script.backend();
    let mut args = request();
    args.systems = vec!["x86_64-linux".into()];
    args.selectors = vec!["static:structure".into()];
    let (status, stdout, _) = invoke_with_mode(args, &root, &cache, &backend, true, true);
    assert_eq!(status, ExitCode::SUCCESS);
    assert_eq!(fs::read_to_string(&sentinel).unwrap(), "unchanged\n");
    let summary: Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(summary["result"]["mode"], "no-write");
    assert_eq!(summary["result"]["counts"]["passed"], 2);
    let record = latest(&root, &cache);
    assert!(record.outcomes.iter().any(|outcome| {
        outcome.id == "nix:x86_64-linux:core:treefmt-check" && outcome.outcome == Outcome::Passed
    }));
    assert!(
        !record
            .outcomes
            .iter()
            .any(|outcome| outcome.id == "format:workspace")
    );
    script.verify().unwrap();
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-012"))]
fn production_no_write_requires_the_native_treefmt_attribute_before_check_work() {
    let (root, cache) = fixture(true);
    let mut script = NixFixture::new(&root);
    discover_calls(&mut script, "x86_64-linux", &["core:unit-tests"]);
    let mut args = request();
    args.systems = vec!["x86_64-linux".into()];
    args.selectors = vec!["static:structure".into()];
    let (status, stdout, _) = invoke_with_mode(args, &root, &cache, &script.backend(), true, true);
    assert_eq!(status, ExitCode::from(2));
    let error: Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(error["error"]["code"], "OperationalError");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("core:treefmt-check")
    );
    assert!(
        RunStore::open(&root, Some(&cache))
            .unwrap()
            .latest()
            .is_err()
    );
    script.verify().unwrap();
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-021"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-023"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-024"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-025"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-026"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-027"))]
fn production_batch_retains_sibling_success_causal_failure_and_log() {
    let (root, cache) = fixture(true);
    let mut script = NixFixture::new(&root);
    let pass = "/nix/store/pass.drv";
    let fail = "/nix/store/fail.drv";
    let dependent = "/nix/store/dependent.drv";
    let data = metadata(&[
        (dependent, "/nix/store/dependent-out", &[fail]),
        (fail, "/nix/store/fail-out", &[]),
        (pass, "/nix/store/pass-out", &[]),
    ]);
    discover_calls(&mut script, "x86_64-linux", &["pass", "fail", "dependent"]);
    formatter(&mut script);
    plan_calls(
        &mut script,
        &[("pass", pass), ("fail", fail), ("dependent", dependent)],
        data.clone(),
    );
    let batch = build(&mut script, &[dependent, fail, pass], false);
    fs::write(batch.join("status"), "1").unwrap();
    fs::write(batch.join("stderr"), format!("error: Cannot build '{fail}'.\n  Reason: builder failed\n  For full logs, run: nix log {fail}\n  > causal builder output\nerror: Cannot build '{dependent}'.\n  Reason: dependency failed\n")).unwrap();
    script.expect(
        &strings(&["derivation", "show", "--recursive", dependent, fail, pass]),
        &data.to_string(),
    );
    validity(
        &mut script,
        &[
            ("/nix/store/dependent-out", false),
            ("/nix/store/fail-out", false),
            ("/nix/store/pass-out", true),
        ],
    );
    let backend = script.backend();
    let (status, _, _) = invoke(selected(), &root, &cache, &backend, true);
    assert_eq!(status, ExitCode::FAILURE);
    let record = latest(&root, &cache);
    assert!(
        record
            .outcomes
            .iter()
            .any(|outcome| outcome.id.ends_with(":pass") && outcome.outcome == Outcome::Passed)
    );
    let failure = record
        .failures
        .iter()
        .find(|failure| failure.check.ends_with(":dependent"))
        .unwrap();
    assert_eq!(failure.nix_log.as_deref(), Some(fail));
    let direct = record
        .failures
        .iter()
        .find(|failure| failure.check.ends_with(":fail"))
        .unwrap();
    assert_eq!(direct.nix_log.as_deref(), Some(fail));
    script.expect(&strings(&["log", fail]), "full causal builder log\n");
    let mut args = request();
    args.operation = Some(CheckOperation::Details(DetailsArgs {
        failure: failure.id.clone(),
        run: None,
        offset: None,
        limit: 100,
    }));
    let (status, stdout, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, ExitCode::SUCCESS);
    assert!(String::from_utf8_lossy(&stdout).contains("full causal builder log"));
    script.verify().unwrap();
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-001"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-002"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-003"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-004"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-006"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-007"))]
fn production_formatter_edits_logs_and_gate_are_observed() {
    for formatter_fails in [false, true] {
        for fail_fast in [false, true] {
            let (root, cache) = fixture(true);
            let mut script = NixFixture::new(&root);
            discover_calls(&mut script, "x86_64-linux", &["one"]);
            let format = formatter(&mut script);
            // Formatting invalidates structure: the static branch must read the edited file.
            fs::write(
                format.join("edit-path"),
                ".bloomery/specs/CLI/CHECK/README.md",
            )
            .unwrap();
            fs::write(
                format.join("edit-contents"),
                "formatted but invalid frontmatter\n",
            )
            .unwrap();
            fs::write(format.join("stderr"), "formatter stderr\n").unwrap();
            if formatter_fails {
                fs::write(format.join("status"), "1").unwrap();
            } else {
                plan_calls(
                    &mut script,
                    &[("one", SHARED)],
                    metadata(&[(SHARED, OUTPUT, &[])]),
                );
                // Select Nix alone in fail-fast mode so a static failure cannot
                // race with admission of the independent batch.
                build(&mut script, &[SHARED], fail_fast);
                validity(&mut script, &[(OUTPUT, true)]);
            }
            let mut args = selected();
            args.fail_fast = fail_fast;
            if !fail_fast {
                args.selectors.clear();
            }
            let (status, stdout, stderr) = invoke(args, &root, &cache, &script.backend(), true);
            assert_eq!(
                status,
                if formatter_fails || !fail_fast {
                    ExitCode::FAILURE
                } else {
                    ExitCode::SUCCESS
                }
            );
            assert_eq!(
                fs::read_to_string(root.join(".bloomery/specs/CLI/CHECK/README.md")).unwrap(),
                "formatted but invalid frontmatter\n"
            );
            let record = latest(&root, &cache);
            let outcome = record
                .outcomes
                .iter()
                .find(|outcome| outcome.id == "format:workspace")
                .unwrap();
            assert_eq!(
                outcome.outcome,
                if formatter_fails {
                    Outcome::Failed
                } else {
                    Outcome::Passed
                }
            );
            let (_, directory) = RunStore::open(&root, Some(&cache))
                .unwrap()
                .latest()
                .unwrap();
            let log = fs::read_to_string(directory.join(&outcome.logs[0])).unwrap();
            assert!(log.contains("formatter stdout") && log.contains("formatter stderr"));
            assert!(!String::from_utf8_lossy(&stdout).contains("formatter stdout"));
            assert!(!String::from_utf8_lossy(&stderr).contains("formatter stderr"));
            if formatter_fails {
                assert!(
                    record
                        .outcomes
                        .iter()
                        .filter(|outcome| outcome.id != "format:workspace")
                        .all(|outcome| outcome.outcome == Outcome::NotRun)
                );
                assert!(
                    record
                        .failures
                        .iter()
                        .any(|failure| failure.code == "NixFormatFailed")
                );
            } else if !fail_fast {
                assert!(
                    record
                        .failures
                        .iter()
                        .any(|failure| failure.check == "static:structure")
                );
            }
            script.verify().unwrap();
        }
    }
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-TESTING-003"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-TESTING-006"))]
fn fixed_output_metadata_uses_the_injected_store_executable() {
    let (root, _) = fixture(true);
    let mut script = NixFixture::new(&root);
    let data = json!({"derivations": {SHARED: {"outputs": {"out": {}}, "inputDrvs": {}}}});
    plan_calls(&mut script, &[("fixed", SHARED)], data);
    script.expect(
        &strings(&["--query", "--outputs", SHARED]),
        &format!("{OUTPUT}\n"),
    );
    let attributes = ["fixed".to_owned()].into();
    let catalog = SystemCatalog::default();
    let request = PlanRequest {
        catalog: &catalog,
        attributes: &attributes,
    };
    let plan = script
        .backend()
        .plan_checks(&root, "x86_64-linux", &request)
        .unwrap();
    assert_eq!(plan.plans[0].outputs, [OUTPUT]);
    script.verify().unwrap();
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-012"))]
fn missing_nix_is_an_operational_runner_error() {
    let (root, cache) = fixture(true);
    let backend = NixCli::with_executable("/missing/bloomery-nix");
    let (status, stdout, _) = invoke(selected(), &root, &cache, &backend, true);
    assert_eq!(status, ExitCode::from(2));
    assert!(String::from_utf8_lossy(&stdout).contains("OperationalError"));
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-TESTING-002"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-TESTING-003"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-TESTING-006"))]
fn strict_fixture_rejects_wrong_arguments_input_and_missing_calls_or_data() {
    for wrong_input in [false, true] {
        let (root, _) = fixture(true);
        let mut script = NixFixture::new(&root);
        let call = script.expect(&strings(&["path-info"]), "{}");
        if wrong_input {
            fs::write(call.join("stdin"), "expected").unwrap();
        }
        assert!(script.verify().is_err(), "unexecuted expectation must fail");
        let mut child = Command::new(NixFixture::executable())
            .args(if wrong_input {
                vec!["path-info"]
            } else {
                vec!["wrong"]
            })
            .current_dir(&root)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        drop(child.stdin.take());
        assert!(!child.wait().unwrap().success());
        assert!(script.verify().is_err());
    }
    let (root, _) = fixture(true);
    let mut script = NixFixture::new(&root);
    let call = script.expect(&[], "");
    fs::remove_file(call.join("status")).unwrap();
    assert!(
        !Command::new(NixFixture::executable())
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );
    assert!(script.verify().is_err());
    let (empty, _) = fixture(true);
    assert!(
        !Command::new(NixFixture::executable())
            .current_dir(empty)
            .status()
            .unwrap()
            .success()
    );
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-TESTING-003"))]
fn independent_fixture_scenarios_can_run_concurrently() {
    std::thread::scope(|scope| {
        for index in 0..3 {
            scope.spawn(move || {
                let (root, _) = fixture(true);
                let mut script = NixFixture::new(&root);
                let name = format!("scenario-{index}");
                discover_calls(&mut script, "x86_64-linux", &[&name]);
                assert_eq!(
                    script
                        .backend()
                        .catalog(&root, "x86_64-linux")
                        .unwrap()
                        .attributes,
                    [name]
                );
                script.verify().unwrap();
            });
        }
    });
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-022"))]
fn fail_fast_batch_omits_keep_going_and_cancels_its_owned_process() {
    let (root, _) = fixture(true);
    let mut script = NixFixture::new(&root);
    let call = build(&mut script, &[SHARED], true);
    fs::write(call.join("block"), "").unwrap();
    let backend = script.backend();
    let cancellation = CancellationToken::new(InterruptFlag::new());
    let (sender, _receiver) = std::sync::mpsc::channel();
    let progress = ProgressSink::new(sender, false);
    let plans = [CheckPlan {
        attribute: "one".into(),
        derivation: Some(SHARED.into()),
        eval_error: None,
        outputs: vec![OUTPUT.into()],
    }];
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            backend.realize_batch(
                &root,
                &plans,
                true,
                &root.join("batch.log"),
                cancellation.clone(),
                &progress,
            )
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while !call.join("blocked").exists() {
            assert!(
                Instant::now() < deadline,
                "fixture never reached the blocked command"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        cancellation.cancel();
        let result = worker.join().unwrap();
        assert!(result.canceled);
        assert!(result.operational_error.is_none());
    });
    assert!(!call.join("finished").exists());
    script.verify().unwrap();
}

const FORMAT_CHECK: &str = "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-treefmt-check.drv";
const FORMAT_OUTPUT: &str = "/nix/store/treefmt-check-out";

fn recorded_catalog(script: &mut NixFixture, system: &str, formatter: Value) {
    record(
        script,
        "bloomeryCatalog",
        "catalog",
        system,
        &json!({
            "checks": ["core:treefmt-check", "one", "two"],
            "legacy": false,
            "formatCheck": {"success": true, "drvPath": FORMAT_CHECK},
            "formatter": formatter,
            "healthSources": null,
        }),
    );
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-033"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-037"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-039"))]
fn complete_no_write_selections_read_recorded_catalog_and_plan() {
    let (root, cache) = fixture(true);
    let mut script = NixFixture::new(&root);
    let two = "/nix/store/two.drv";
    recorded_catalog(&mut script, "x86_64-linux", Value::Null);
    record(
        &mut script,
        "bloomeryPlan",
        "plan",
        "x86_64-linux",
        &json!({"legacy": false, "plan": {
            "one": {"success": true, "drvPath": SHARED},
            "two": {"success": true, "drvPath": two},
        }}),
    );
    let data = metadata(&[
        (FORMAT_CHECK, FORMAT_OUTPUT, &[]),
        (SHARED, OUTPUT, &[]),
        (two, "/nix/store/two-out", &[]),
    ]);
    script.expect(
        &strings(&["derivation", "show", FORMAT_CHECK, SHARED, two]),
        &data.to_string(),
    );
    build(&mut script, &[FORMAT_CHECK, SHARED, two], false);
    validity(
        &mut script,
        &[
            (FORMAT_OUTPUT, true),
            (OUTPUT, true),
            ("/nix/store/two-out", true),
        ],
    );
    let mut args = request();
    args.systems = vec!["x86_64-linux".into()];
    // Every Nix check is selected; static checks would race with the fixture
    // writing its call records inside the workspace.
    args.selectors = vec!["nix:*".into()];
    let (status, stdout, stderr) =
        invoke_with_mode(args, &root, &cache, &script.backend(), true, true);
    assert_eq!(
        status,
        ExitCode::SUCCESS,
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr)
    );
    let record = latest(&root, &cache);
    for name in ["core:treefmt-check", "one", "two"] {
        assert!(record.outcomes.iter().any(|outcome| {
            outcome.id == format!("nix:x86_64-linux:{name}") && outcome.outcome == Outcome::Passed
        }));
    }
    script.verify().unwrap();
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-038"))]
fn partial_recorded_selections_evaluate_only_selected_attributes() {
    let (root, cache) = fixture(true);
    let mut script = NixFixture::new(&root);
    recorded_catalog(&mut script, "x86_64-linux", Value::Null);
    // The catalog's formatter check is reused; only `one` is evaluated.
    script.expect(
        &plan_args("x86_64-linux", &["one"]),
        &json!({"legacy": false, "plan": {"one": {"success": true, "drvPath": SHARED}}})
            .to_string(),
    );
    script.expect(
        &strings(&["derivation", "show", FORMAT_CHECK, SHARED]),
        &metadata(&[(FORMAT_CHECK, FORMAT_OUTPUT, &[]), (SHARED, OUTPUT, &[])]).to_string(),
    );
    build(&mut script, &[FORMAT_CHECK, SHARED], false);
    validity(&mut script, &[(FORMAT_OUTPUT, true), (OUTPUT, true)]);
    let mut args = request();
    args.systems = vec!["x86_64-linux".into()];
    args.selectors = vec!["nix:*:one".into()];
    let (status, _, _) = invoke_with_mode(args, &root, &cache, &script.backend(), true, true);
    assert_eq!(status, ExitCode::SUCCESS);
    script.verify().unwrap();
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-013"))]
fn recorded_formatters_are_realized_and_run_without_nix_fmt() {
    let (root, cache) = fixture(true);
    let mut script = NixFixture::new(&root);
    let formatter = "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-treefmt.drv";
    recorded_catalog(
        &mut script,
        &host(),
        json!({"drvPath": formatter, "program": NixFixture::executable()}),
    );
    script.expect(
        &strings(&["build", "--no-link", &format!("{formatter}^out")]),
        "",
    );
    // The fixture executable stands in for the recorded program; it is
    // invoked with no arguments from the workspace root.
    script.expect(&[], "formatted\n");
    let mut args = request();
    args.selectors = vec!["static:structure".into()];
    let (status, _, _) = invoke(args, &root, &cache, &script.backend(), true);
    assert_eq!(status, ExitCode::SUCCESS);
    assert!(
        latest(&root, &cache).outcomes.iter().any(|outcome| {
            outcome.id == "format:workspace" && outcome.outcome == Outcome::Passed
        })
    );
    script.verify().unwrap();
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-036"))]
fn catalog_read_failures_other_than_a_missing_attribute_are_operational() {
    let (root, cache) = fixture(true);
    let mut script = NixFixture::new(&root);
    let failed = script.expect(&record_args("bloomeryCatalog", "x86_64-linux"), "");
    fs::write(failed.join("status"), "1").unwrap();
    fs::write(
        failed.join("stderr"),
        "error: syntax error, unexpected '}'\n",
    )
    .unwrap();
    let mut args = request();
    args.systems = vec!["x86_64-linux".into()];
    let (status, stdout, _) = invoke(args, &root, &cache, &script.backend(), true);
    assert_eq!(status, ExitCode::from(2));
    let error: Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(error["error"]["code"], "OperationalError");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("check catalog read failed")
    );
    script.verify().unwrap();
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-FORMAT-014"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-034"))]
fn generic_flakes_discover_with_nix_eval_and_format_with_nix_fmt() {
    let (root, cache) = fixture(true);
    let mut script = NixFixture::new(&root);
    // The missing catalog record selects attribute-name evaluation and `nix fmt`.
    discover_calls(&mut script, &host(), &["one"]);
    formatter(&mut script);
    let mut args = request();
    args.selectors = vec!["static:structure".into()];
    let (status, _, _) = invoke(args, &root, &cache, &script.backend(), true);
    assert_eq!(status, ExitCode::SUCCESS);
    script.verify().unwrap();
}
