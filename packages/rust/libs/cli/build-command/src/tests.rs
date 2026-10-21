use super::*;
use crate::backend::{
    NixBackend, NixCli, PackagePlan, Realization, parse_build_result, parse_derivation_show_json,
    parse_packages_json, parse_path_info_json, plan_args, realize_args,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

fn root() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "bloomery-build-{}-{}",
        std::process::id(),
        NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).expect("temp root");
    path
}

fn flake_root() -> PathBuf {
    let path = root();
    std::fs::write(path.join("flake.nix"), "{ }\n").expect("flake");
    path
}

fn drv(name: &str) -> String {
    format!("/nix/store/{name}.drv")
}

fn out(name: &str) -> String {
    format!("/nix/store/{name}")
}

fn plan(name: &str, system: &str) -> PackagePlan {
    PackagePlan {
        name: name.to_owned(),
        system: system.to_owned(),
        derivation: Some(drv(name)),
        eval_error: None,
    }
}

#[derive(Default)]
struct FakeBackend {
    host: String,
    packages: BTreeMap<String, Vec<PackagePlan>>,
    plan_errors: BTreeMap<String, String>,
    realization: Realization,
    outputs: BTreeMap<String, Vec<String>>,
    valid: BTreeSet<String>,
    plan_calls: Mutex<Vec<String>>,
    realize_calls: Mutex<Vec<Vec<String>>>,
}

impl FakeBackend {
    fn host_system(&self) -> String {
        if self.host.is_empty() {
            "x86_64-linux".to_owned()
        } else {
            self.host.clone()
        }
    }
}

impl NixBackend for FakeBackend {
    fn host_system(&self, _root: &Path) -> Result<String, String> {
        Ok(FakeBackend::host_system(self))
    }

    fn plan_packages(&self, _root: &Path, system: &str) -> Result<Vec<PackagePlan>, String> {
        self.plan_calls.lock().unwrap().push(system.to_owned());
        if let Some(error) = self.plan_errors.get(system) {
            return Err(error.clone());
        }
        Ok(self.packages.get(system).cloned().unwrap_or_default())
    }

    fn realize(&self, _root: &Path, derivations: &[String]) -> Realization {
        self.realize_calls
            .lock()
            .unwrap()
            .push(derivations.to_vec());
        self.realization.clone()
    }

    fn output_paths(
        &self,
        _root: &Path,
        _derivations: &[String],
    ) -> Result<BTreeMap<String, Vec<String>>, String> {
        Ok(self.outputs.clone())
    }

    fn validate_outputs(
        &self,
        _root: &Path,
        _outputs: &[String],
    ) -> Result<BTreeSet<String>, String> {
        Ok(self.valid.clone())
    }
}

#[derive(Default)]
struct FakeGate {
    outcome: GateOutcome,
    calls: Mutex<Vec<Vec<String>>>,
    no_write_calls: Mutex<Vec<bool>>,
}

impl CheckGate for FakeGate {
    fn run(&self, _root: &Path, systems: &[String], no_write: bool) -> GateOutcome {
        self.calls.lock().unwrap().push(systems.to_vec());
        self.no_write_calls.lock().unwrap().push(no_write);
        self.outcome.clone()
    }
}

fn passed_backend(names: &[&str]) -> FakeBackend {
    let system = "x86_64-linux";
    let packages = names.iter().map(|name| plan(name, system)).collect();
    let outputs = names
        .iter()
        .map(|name| (drv(name), vec![out(name)]))
        .collect();
    let valid = names.iter().map(|name| out(name)).collect();
    FakeBackend {
        host: system.to_owned(),
        packages: BTreeMap::from([(system.to_owned(), packages)]),
        outputs,
        valid,
        ..FakeBackend::default()
    }
}

fn default_args() -> BuildArgs {
    BuildArgs::default()
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-COMMAND-004"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-COMMAND-008"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-COMMAND-013"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-001"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-007"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-008"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-010"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-011"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-012"))]
fn executes_default_selection_for_the_host_system() {
    let backend = passed_backend(&["app", "tool"]);
    let report = execute(
        default_args(),
        Path::new("."),
        &backend,
        &FakeGate::default(),
    )
    .expect("build report");
    assert_eq!(report.status, BuildStatus::Passed);
    assert_eq!(report.systems, vec!["x86_64-linux".to_owned()]);
    assert_eq!(report.packages.len(), 2);
    assert!(
        report
            .packages
            .iter()
            .all(|package| package.outcome == Outcome::Passed)
    );
    assert_eq!(report.packages[0].path.as_deref(), Some("/nix/store/app"));
    assert_eq!(backend.plan_calls.lock().unwrap().len(), 1);
    assert_eq!(backend.realize_calls.lock().unwrap().len(), 1);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-COMMAND-005"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-COMMAND-006"))]
fn package_selectors_form_a_deduplicated_union() {
    let backend = passed_backend(&["app", "lib", "tool"]);
    let args = BuildArgs {
        packages: vec!["app".to_owned(), "a*".to_owned(), "app".to_owned()],
        ..BuildArgs::default()
    };
    let report = execute(args, Path::new("."), &backend, &FakeGate::default()).expect("report");
    let names = report
        .packages
        .iter()
        .map(|package| package.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["app"]);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-COMMAND-007"))]
fn unmatched_package_selectors_are_rejected() {
    let backend = passed_backend(&["app"]);
    let args = BuildArgs {
        packages: vec!["missing*".to_owned()],
        ..BuildArgs::default()
    };
    let error = execute(args, Path::new("."), &backend, &FakeGate::default()).expect_err("usage");
    assert_eq!(error.kind(), BuildErrorKind::Usage);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-COMMAND-009"))]
fn explicit_systems_are_deduplicated() {
    let mut backend = passed_backend(&["app"]);
    backend.packages.insert(
        "aarch64-linux".to_owned(),
        vec![plan("app", "aarch64-linux")],
    );
    backend.outputs.insert(drv("app"), vec![out("app")]);
    backend.valid.insert(out("app"));
    let args = BuildArgs {
        systems: vec![
            "x86_64-linux".to_owned(),
            "x86_64-linux".to_owned(),
            "aarch64-linux".to_owned(),
        ],
        ..BuildArgs::default()
    };
    let report = execute(args, Path::new("."), &backend, &FakeGate::default()).expect("report");
    assert_eq!(
        report.systems,
        vec!["x86_64-linux".to_owned(), "aarch64-linux".to_owned()]
    );
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-COMMAND-010"))]
fn explicit_missing_system_is_an_error() {
    let mut backend = passed_backend(&["app"]);
    backend
        .plan_errors
        .insert("aarch64-linux".to_owned(), "no packages output".to_owned());
    let args = BuildArgs {
        systems: vec!["aarch64-linux".to_owned()],
        ..BuildArgs::default()
    };
    let error = execute(args, Path::new("."), &backend, &FakeGate::default()).expect_err("error");
    assert_eq!(error.kind(), BuildErrorKind::Operational);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-COMMAND-011"))]
fn empty_package_set_is_valid() {
    let mut backend = passed_backend(&["app"]);
    backend
        .packages
        .insert("aarch64-linux".to_owned(), Vec::new());
    let args = BuildArgs {
        systems: vec!["x86_64-linux".to_owned(), "aarch64-linux".to_owned()],
        ..BuildArgs::default()
    };
    let report = execute(args, Path::new("."), &backend, &FakeGate::default()).expect("report");
    assert_eq!(report.status, BuildStatus::Passed);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-COMMAND-012"))]
fn empty_selection_is_a_usage_error() {
    let backend = FakeBackend {
        host: "x86_64-linux".to_owned(),
        ..FakeBackend::default()
    };
    let error = execute(
        default_args(),
        Path::new("."),
        &backend,
        &FakeGate::default(),
    )
    .expect_err("usage");
    assert_eq!(error.kind(), BuildErrorKind::Usage);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-COMMAND-014"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-013"))]
fn failed_package_returns_failure_and_retains_derivation() {
    let mut backend = passed_backend(&["app"]);
    backend
        .realization
        .failed
        .insert(drv("app"), "build failed".to_owned());
    let report = execute(
        default_args(),
        Path::new("."),
        &backend,
        &FakeGate::default(),
    )
    .expect("report");
    assert_eq!(report.status, BuildStatus::Failed);
    assert_eq!(report.packages[0].outcome, Outcome::Failed);
    assert_eq!(report.packages[0].derivation.as_deref(), Some(&*drv("app")));
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-COMMAND-015"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-015"))]
fn missing_nix_is_operational() {
    let mut backend = passed_backend(&["app"]);
    backend.realization.operational_error = Some("unable to run nix".to_owned());
    let error = execute(
        default_args(),
        Path::new("."),
        &backend,
        &FakeGate::default(),
    )
    .expect_err("operational");
    assert_eq!(error.kind(), BuildErrorKind::Operational);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-COMMAND-016"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-016"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-017"))]
fn cancellation_distinguishes_canceled_and_not_run() {
    let mut backend = passed_backend(&["app", "tool"]);
    // `app` completes, `tool` is canceled, and a third package never starts.
    backend.realization.canceled.insert(drv("tool"));
    backend
        .packages
        .get_mut("x86_64-linux")
        .unwrap()
        .push(plan("queued", "x86_64-linux"));
    backend.realization.not_run.insert(drv("queued"));
    let report = execute(
        default_args(),
        Path::new("."),
        &backend,
        &FakeGate::default(),
    )
    .expect("report");
    assert_eq!(report.status, BuildStatus::Interrupted);
    let outcomes = report
        .packages
        .iter()
        .map(|package| (package.name.as_str(), package.outcome))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(outcomes["app"], Outcome::Passed);
    assert_eq!(outcomes["tool"], Outcome::Canceled);
    assert_eq!(outcomes["queued"], Outcome::NotRun);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-005"))]
fn evaluation_failures_are_isolated() {
    let backend = FakeBackend {
        host: "x86_64-linux".to_owned(),
        packages: BTreeMap::from([(
            "x86_64-linux".to_owned(),
            vec![
                plan("app", "x86_64-linux"),
                PackagePlan {
                    name: "broken".to_owned(),
                    system: "x86_64-linux".to_owned(),
                    derivation: None,
                    eval_error: Some("broken did not evaluate to a derivation".to_owned()),
                },
            ],
        )]),
        outputs: BTreeMap::from([(drv("app"), vec![out("app")])]),
        valid: BTreeSet::from([out("app")]),
        ..FakeBackend::default()
    };
    let report = execute(
        default_args(),
        Path::new("."),
        &backend,
        &FakeGate::default(),
    )
    .expect("report");
    assert_eq!(report.status, BuildStatus::Failed);
    assert_eq!(backend.realize_calls.lock().unwrap()[0], vec![drv("app")]);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-003"))]
fn library_packages_are_built() {
    let backend = passed_backend(&["app", "lib:lib"]);
    let report = execute(
        default_args(),
        Path::new("."),
        &backend,
        &FakeGate::default(),
    )
    .expect("report");
    assert!(
        report
            .packages
            .iter()
            .any(|package| package.name == "lib:lib")
    );
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-002"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-004"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-006"))]
fn plan_evaluates_the_packages_output() {
    let args = plan_args("x86_64-linux");
    let args = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(args.contains(&"eval".to_owned()));
    assert!(args.contains(&".#packages.x86_64-linux".to_owned()));
    assert!(args.contains(&"--no-write-lock-file".to_owned()));
    assert!(args.contains(&"--no-update-lock-file".to_owned()));
    assert!(!args.contains(&"--impure".to_owned()));
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-007"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-009"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-014"))]
fn realize_args_are_link_safe_and_keep_going() {
    let args = realize_args(&[drv("app")]);
    let args = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(args.contains(&"build".to_owned()));
    assert!(args.contains(&"--no-link".to_owned()));
    assert!(args.contains(&"--keep-going".to_owned()));
    assert!(args.iter().any(|arg| arg.ends_with("^*")));
    assert!(!args.iter().any(|arg| arg.starts_with("--out-link")));
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-COMMAND-017"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-021"))]
fn gate_uses_selected_systems() {
    let gate = FakeGate::default();
    let args = BuildArgs {
        systems: vec!["aarch64-linux".to_owned()],
        check: true,
        ..BuildArgs::default()
    };
    let mut backend = passed_backend(&["app"]);
    backend.packages.insert(
        "aarch64-linux".to_owned(),
        vec![plan("app", "aarch64-linux")],
    );
    let report = execute(args, Path::new("."), &backend, &gate).expect("report");
    assert_eq!(
        gate.calls.lock().unwrap()[0],
        vec!["aarch64-linux".to_owned()]
    );
    assert_eq!(
        report.gate.expect("gate report").status,
        BuildStatus::Passed
    );
    assert_eq!(gate.no_write_calls.lock().unwrap().as_slice(), &[false]);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-020"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-028"))]
fn no_write_build_forwards_mode_to_the_gate_and_preserves_workspace_files() {
    let root = flake_root();
    let cargo = root.join("Cargo.toml");
    let config = root.join("config.toml");
    let lock = root.join("flake.lock");
    let baseline = root.join("health-summary.json");
    for (path, bytes) in [
        (&cargo, b"workspace source\n".as_slice()),
        (&config, b"workspace config\n".as_slice()),
        (&lock, b"flake lock\n".as_slice()),
        (&baseline, b"health baseline\n".as_slice()),
    ] {
        std::fs::write(path, bytes).expect("workspace file");
    }
    let before = [&cargo, &config, &lock, &baseline]
        .into_iter()
        .map(|path| std::fs::read(path).expect("read workspace file"))
        .collect::<Vec<_>>();
    let gate = FakeGate {
        outcome: GateOutcome {
            status: BuildStatus::Passed,
            ..GateOutcome::default()
        },
        ..FakeGate::default()
    };
    let backend = passed_backend(&["app"]);
    let args = BuildArgs {
        check: true,
        ..BuildArgs::default()
    };
    let report = execute_with_mode(args, &root, &backend, &gate, true).expect("build report");
    assert_eq!(report.status, BuildStatus::Passed);
    assert_eq!(gate.no_write_calls.lock().unwrap().as_slice(), &[true]);
    assert_eq!(backend.realize_calls.lock().unwrap().len(), 1);
    assert_eq!(
        [&cargo, &config, &lock, &baseline]
            .into_iter()
            .map(|path| std::fs::read(path).expect("workspace file unchanged"))
            .collect::<Vec<_>>(),
        before
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-022"))]
fn gate_pass_proceeds_to_realization() {
    let gate = FakeGate {
        outcome: GateOutcome {
            status: BuildStatus::Passed,
            ..GateOutcome::default()
        },
        ..FakeGate::default()
    };
    let backend = passed_backend(&["app"]);
    let args = BuildArgs {
        check: true,
        ..BuildArgs::default()
    };
    let report = execute(args, Path::new("."), &backend, &gate).expect("report");
    assert_eq!(report.status, BuildStatus::Passed);
    assert_eq!(backend.realize_calls.lock().unwrap().len(), 1);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-023"))]
fn gate_failure_blocks_realization() {
    let gate = FakeGate {
        outcome: GateOutcome {
            status: BuildStatus::Failed,
            failed_checks: vec!["app:test".to_owned()],
            ..GateOutcome::default()
        },
        ..FakeGate::default()
    };
    let backend = passed_backend(&["app"]);
    let args = BuildArgs {
        check: true,
        ..BuildArgs::default()
    };
    let report = execute(args, Path::new("."), &backend, &gate).expect("report");
    assert_eq!(report.status, BuildStatus::Failed);
    assert!(backend.realize_calls.lock().unwrap().is_empty());
    assert_eq!(
        report.gate.expect("gate report").failed_checks,
        vec!["app:test"]
    );
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-025"))]
fn gate_operational_failure_returns_error() {
    let gate = FakeGate {
        outcome: GateOutcome {
            status: BuildStatus::Error,
            ..GateOutcome::default()
        },
        ..FakeGate::default()
    };
    let backend = passed_backend(&["app"]);
    let args = BuildArgs {
        check: true,
        ..BuildArgs::default()
    };
    let report = execute(args, Path::new("."), &backend, &gate).expect("report");
    assert_eq!(report.status, BuildStatus::Error);
    assert!(backend.realize_calls.lock().unwrap().is_empty());
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-018"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-019"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-020"))]
fn build_without_check_skips_gate() {
    let gate = FakeGate::default();
    let backend = passed_backend(&["app"]);
    let report = execute(default_args(), Path::new("."), &backend, &gate).expect("report");
    assert!(report.gate.is_none());
    assert!(gate.calls.lock().unwrap().is_empty());
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-026"))]
fn build_executes_without_a_specification_tree() {
    let root = flake_root();
    let backend = passed_backend(&["app"]);
    let report = execute(default_args(), &root, &backend, &FakeGate::default()).expect("report");
    assert_eq!(report.status, BuildStatus::Passed);
    assert_eq!(backend.realize_calls.lock().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-OUTPUT-001"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-OUTPUT-002"))]
#[cfg_attr(any(), bloomery("CLI-INTERFACE-PRESENTATION-014"))]
fn human_report_lists_output_paths() {
    let backend = passed_backend(&["app"]);
    let root = flake_root();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_at_with(
        default_args(),
        false,
        ColorMode::Never,
        &root,
        &backend,
        &FakeGate::default(),
        (&mut stdout, &mut stderr),
    );
    assert_eq!(status, std::process::ExitCode::from(0));
    let text = String::from_utf8(stdout).expect("stdout");
    assert!(text.contains("app"), "{text}");
    assert!(text.contains("/nix/store/app"), "{text}");
    assert_eq!(text.trim_end().lines().last(), Some("✓ passed · 1 package"));
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-OUTPUT-002"))]
#[cfg_attr(any(), bloomery("CLI-INTERFACE-PRESENTATION-014"))]
fn human_build_summary_omits_zero_outcome_counts() {
    let mut backend = passed_backend(&["app"]);
    backend
        .realization
        .failed
        .insert(drv("app"), "build failed".to_owned());
    let root = flake_root();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_at_with(
        default_args(),
        false,
        ColorMode::Never,
        &root,
        &backend,
        &FakeGate::default(),
        (&mut stdout, &mut stderr),
    );
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let text = String::from_utf8(stdout).expect("human output");
    assert_eq!(text.trim_end().lines().last(), Some("✗ failed · 1 failed"));
    assert!(!text.contains("0 passed"), "{text}");

    let mut backend = passed_backend(&["passed", "failed"]);
    backend
        .realization
        .failed
        .insert(drv("failed"), "build failed".to_owned());
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_at_with(
        default_args(),
        false,
        ColorMode::Never,
        &root,
        &backend,
        &FakeGate::default(),
        (&mut stdout, &mut stderr),
    );
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let text = String::from_utf8(stdout).expect("human output");
    assert_eq!(
        text.trim_end().lines().last(),
        Some("✗ failed · 1 passed · 1 failed")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-OUTPUT-005"))]
fn failed_package_names_log_retrieval() {
    let mut backend = passed_backend(&["app"]);
    backend
        .realization
        .failed
        .insert(drv("app"), "build failed".to_owned());
    let root = flake_root();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_at_with(
        default_args(),
        false,
        ColorMode::Auto,
        &root,
        &backend,
        &FakeGate::default(),
        (&mut stdout, &mut stderr),
    );
    let text = String::from_utf8(stderr).expect("stderr");
    assert!(text.contains("nix log"), "{text}");
    assert!(text.contains(&drv("app")), "{text}");
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-OUTPUT-003"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-OUTPUT-009"))]
fn json_report_is_structured() {
    let backend = passed_backend(&["app"]);
    let root = flake_root();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let args = BuildArgs {
        check: true,
        ..BuildArgs::default()
    };
    run_at_with(
        args,
        true,
        ColorMode::Auto,
        &root,
        &backend,
        &FakeGate::default(),
        (&mut stdout, &mut stderr),
    );
    let value: serde_json::Value = serde_json::from_slice(&stdout).expect("json");
    assert_eq!(value["command"], "build");
    assert_eq!(value["status"], "succeeded");
    assert_eq!(value["result"]["status"], "passed");
    assert_eq!(value["result"]["packages"][0]["name"], "app");
    assert_eq!(value["result"]["packages"][0]["path"], "/nix/store/app");
    assert_eq!(value["result"]["checks"]["status"], "passed");
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-OUTPUT-004"))]
fn json_usage_error_is_structured() {
    let backend = passed_backend(&["app"]);
    let root = flake_root();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let args = BuildArgs {
        packages: vec!["missing".to_owned()],
        ..BuildArgs::default()
    };
    run_at_with(
        args,
        true,
        ColorMode::Auto,
        &root,
        &backend,
        &FakeGate::default(),
        (&mut stdout, &mut stderr),
    );
    let value: serde_json::Value = serde_json::from_slice(&stdout).expect("json");
    assert_eq!(value["status"], "error");
    assert_eq!(value["error"]["kind"], "usage");
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-OUTPUT-006"))]
fn json_output_has_no_progress_or_color() {
    let backend = passed_backend(&["app"]);
    let root = flake_root();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_at_with(
        default_args(),
        true,
        ColorMode::Auto,
        &root,
        &backend,
        &FakeGate::default(),
        (&mut stdout, &mut stderr),
    );
    let text = String::from_utf8(stdout).expect("stdout");
    assert!(!text.contains("\u{1b}["), "{text}");
    assert!(!text.contains("building"), "{text}");
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-OUTPUT-007"))]
fn raw_logs_are_not_streamed() {
    let mut backend = passed_backend(&["app"]);
    backend
        .realization
        .failed
        .insert(drv("app"), "RAW-BUILD-LOG-SENTINEL".to_owned());
    let root = flake_root();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_at_with(
        default_args(),
        false,
        ColorMode::Auto,
        &root,
        &backend,
        &FakeGate::default(),
        (&mut stdout, &mut stderr),
    );
    // The failure is summarized, but the retained derivation is not expanded.
    assert!(
        !String::from_utf8(stdout)
            .unwrap()
            .contains("RAW-BUILD-LOG-SENTINEL")
    );
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-OUTPUT-008"))]
fn gate_failure_is_reported() {
    let gate = FakeGate {
        outcome: GateOutcome {
            status: BuildStatus::Failed,
            failed_checks: vec!["app:test".to_owned()],
            ..GateOutcome::default()
        },
        ..FakeGate::default()
    };
    let backend = passed_backend(&["app"]);
    let root = flake_root();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let args = BuildArgs {
        check: true,
        ..BuildArgs::default()
    };
    run_at_with(
        args,
        false,
        ColorMode::Auto,
        &root,
        &backend,
        &gate,
        (&mut stdout, &mut stderr),
    );
    let text = String::from_utf8(stdout).expect("stdout");
    assert!(text.contains("checks failed"), "{text}");
    assert!(text.contains("app:test"), "{text}");
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-COMMAND-008"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-006"))]
fn host_system_resolves_without_impure_eval() {
    let system = NixCli::default()
        .host_system(Path::new("."))
        .expect("host system resolves from nix.conf or the target triple");
    assert!(system.contains('-'), "unexpected host system '{system}'");
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-002"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-005"))]
fn packages_json_parses_success_and_failure() {
    let bytes = br#"{
        "app": { "success": true, "drvPath": "/nix/store/app.drv" },
        "broken": { "success": false }
    }"#;
    let plans = parse_packages_json("x86_64-linux", bytes).expect("plans");
    assert_eq!(plans.len(), 2);
    let app = plans.iter().find(|plan| plan.name == "app").expect("app");
    assert_eq!(app.derivation.as_deref(), Some("/nix/store/app.drv"));
    let broken = plans
        .iter()
        .find(|plan| plan.name == "broken")
        .expect("broken");
    assert!(broken.derivation.is_none());
    assert!(broken.eval_error.is_some());
    assert!(parse_packages_json("x86_64-linux", b"not json").is_err());
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-009"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-013"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-015"))]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-016"))]
fn build_result_parses_failure_cancellation_and_operational_error() {
    let derivations = vec![drv("app")];
    let success = parse_build_result(&derivations, true, Some(0), "");
    assert!(success.failed.is_empty());
    assert!(success.canceled.is_empty());

    let canceled = parse_build_result(&derivations, false, Some(130), "");
    assert!(canceled.canceled.contains(&drv("app")));

    let stderr = format!("error: Cannot build '{}'\n", drv("app"));
    let failed = parse_build_result(&derivations, false, Some(1), &stderr);
    assert!(failed.failed.contains_key(&drv("app")));

    let operational = parse_build_result(&derivations, false, Some(1), "network down");
    assert!(operational.operational_error.is_some());
}

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-012"))]
fn derivation_show_and_path_info_parse_outputs() {
    // `nix derivation show` returns a `derivations` wrapper keyed by the
    // store basename, and output paths are basenames too.
    let derivation_bytes = format!(
        r#"{{ "version": 4, "derivations": {{ "{}.drv": {{ "outputs": {{ "out": {{ "path": "{}" }} }} }} }} }}"#,
        drv("app")
            .trim_start_matches("/nix/store/")
            .trim_end_matches(".drv"),
        out("app").trim_start_matches("/nix/store/")
    );
    let paths = parse_derivation_show_json(derivation_bytes.as_bytes()).expect("paths");
    assert_eq!(paths.get(&drv("app")), Some(&vec![out("app")]));

    // Older direct-object output is still accepted.
    let legacy = format!(
        r#"{{ "{}": {{ "outputs": {{ "out": {{ "path": "{}" }} }} }} }}"#,
        drv("app"),
        out("app")
    );
    let legacy_paths = parse_derivation_show_json(legacy.as_bytes()).expect("legacy paths");
    assert_eq!(legacy_paths.get(&drv("app")), Some(&vec![out("app")]));

    let path_info = format!(r#"{{ "{}": {{ "narSize": 1 }} }}"#, out("app"));
    let valid = parse_path_info_json(path_info.as_bytes()).expect("valid");
    assert!(valid.contains(&out("app")));

    assert!(parse_derivation_show_json(b"[]").is_err());
    assert!(parse_path_info_json(b"[]").is_err());
}
