use crate::check_command::catalog::{
    CheckPlan, NIX_DISCOVER_EXPRESSION, NIX_PLAN_EXPRESSION, NixBackend, NixCli,
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

fn eval_args(system: &str, planning: bool) -> Vec<String> {
    vec![
        "eval".into(),
        "--no-write-lock-file".into(),
        "--no-update-lock-file".into(),
        "--json".into(),
        format!(".#checks.{system}"),
        "--apply".into(),
        if planning {
            NIX_PLAN_EXPRESSION
        } else {
            NIX_DISCOVER_EXPRESSION
        }
        .into(),
    ]
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
    script.expect(
        &eval_args("x86_64-linux", true),
        &json!({"legacy": false, "plan": plans}).to_string(),
    );
    let mut args = strings(&["derivation", "show"]);
    let mut names = names.to_vec();
    names.sort_by_key(|(name, _)| *name);
    args.extend(names.iter().map(|(_, drv)| (*drv).to_owned()));
    script.expect(&args, &metadata.to_string());
}

fn discover_calls(script: &mut NixFixture, system: &str, names: &[&str]) {
    let mut names = names.to_vec();
    names.sort();
    script.expect(&eval_args(system, false), &json!(names).to_string());
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
    script.expect(
        &eval_args("x86_64-linux", false),
        &json!(sorted).to_string(),
    );
    let backend = script.backend();
    assert_eq!(backend.discover(&root, "x86_64-linux").unwrap(), sorted);
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
    let missing = script.expect(&eval_args("aarch64-darwin", false), "");
    fs::write(missing.join("status"), "1").unwrap();
    fs::write(missing.join("stderr"), "missing checks output\n").unwrap();
    let mut args = request();
    args.systems = vec!["aarch64-darwin".into()];
    let (status, stdout, _) = invoke(args, &root, &cache, &script.backend(), true);
    assert_eq!(status, ExitCode::from(2));
    let error: Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(error["error"]["code"], "OperationalError");
    discover_calls(&mut script, "aarch64-linux", &[]);
    formatter(&mut script);
    script.expect(
        &eval_args("aarch64-linux", true),
        r#"{"legacy":false,"plan":{}}"#,
    );
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
    script.expect(
        &eval_args("x86_64-linux", true),
        r#"{"legacy":false,"plan":{}}"#,
    );
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
    let plan = script.backend().plan_checks(&root, "x86_64-linux").unwrap();
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
                script.expect(
                    &eval_args("x86_64-linux", false),
                    &json!([format!("scenario-{index}")]).to_string(),
                );
                assert_eq!(
                    script.backend().discover(&root, "x86_64-linux").unwrap(),
                    [format!("scenario-{index}")]
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
