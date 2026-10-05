use crate::check_command::catalog::{NixBackend, NixCli};
use crate::check_command::command::{
    CheckArgs, CheckOperation, FailureArgs, ListArgs, validate_args,
};
use crate::check_command::execution::validate_execution_selection;
use crate::check_command::test_support::{
    FakeNix, fixture, invoke, lock_fixture_flake, nix_is_available, request,
};
use serde_json::Value;
use std::fs;
use std::sync::atomic::Ordering;

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-001"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-010"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-012"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-NIX-018"))]
fn default_and_explicit_selection_choose_static_and_requested_system_catalogs() {
    let (root, cache) = fixture(true);
    let static_backend = FakeNix::default();
    let (status, stdout, _) = invoke(request(), &root, &cache, &static_backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let result: Value = serde_json::from_slice(&stdout).expect("summary JSON");
    // Two static checks plus the implicit format:workspace gate; one catalog
    // evaluation resolves the (empty) Nix plan.
    assert_eq!(result["counts"]["passed"], 3);
    assert_eq!(static_backend.evaluations.load(Ordering::SeqCst), 1);
    assert_eq!(static_backend.formatter_runs.load(Ordering::SeqCst), 1);

    let (root, cache2) = fixture(true);
    let backend = FakeNix {
        names: vec!["core:test".to_owned()],
        ..FakeNix::default()
    };
    let mut args = request();
    args.systems = vec![
        "aarch64-linux".to_owned(),
        "x86_64-linux".to_owned(),
        "aarch64-linux".to_owned(),
    ];
    args.selectors = vec!["nix:*:core:test".to_owned()];
    args.jobs = Some(2);
    let (status, stdout, _) = invoke(args, &root, &cache2, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    assert_eq!(backend.systems.lock().unwrap().len(), 2);
    // One catalog evaluation per selected system resolves both IDs and plans.
    assert_eq!(backend.evaluations.load(Ordering::SeqCst), 2);
    let result: Value = serde_json::from_slice(&stdout).expect("partial run JSON");
    assert_eq!(result["scope"]["checks"][0], "nix:*:core:test");
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
    let _ = fs::remove_dir_all(cache2);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-013"))]
fn default_check_execution_without_a_flake_returns_the_shared_setup_error() {
    let (root, cache) = fixture(false);
    let backend = FakeNix::default();
    let (status, stdout, _) = invoke(request(), &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::from(2));
    let error: Value = serde_json::from_slice(&stdout).expect("setup error");
    assert_eq!(error["error"]["code"], "MissingFlake");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("flake.nix")
    );
    assert_eq!(backend.formatter_runs.load(Ordering::SeqCst), 0);
    assert_eq!(backend.evaluations.load(Ordering::SeqCst), 0);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-014"))]
fn execution_selection_rejects_an_empty_catalog() {
    assert_eq!(
        validate_execution_selection(&[]).unwrap_err().code,
        "UsageError"
    );
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-015"))]
fn explicit_nix_selection_without_a_flake_is_an_error() {
    let (root, cache) = fixture(false);
    let mut args = request();
    args.selectors = vec!["nix:*".to_owned()];
    let backend = FakeNix::default();
    let (status, stdout, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::from(2));
    let error: Value = serde_json::from_slice(&stdout).expect("setup error");
    assert_eq!(error["error"]["code"], "MissingFlake");
    assert_eq!(backend.formatter_runs.load(Ordering::SeqCst), 0);
    assert_eq!(backend.evaluations.load(Ordering::SeqCst), 0);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-011"))]
fn explicitly_requested_systems_without_a_checks_output_are_errors() {
    let (root, cache) = fixture(true);
    let backend = FakeNix {
        discovery_error: Some("selected system has no checks output".to_owned()),
        ..FakeNix::default()
    };
    let mut args = request();
    args.systems = vec!["aarch64-darwin".to_owned()];
    let (status, stdout, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::from(2));
    let error: Value = serde_json::from_slice(&stdout).expect("discovery error");
    assert_eq!(error["error"]["code"], "OperationalError");
    assert_eq!(
        backend.systems.lock().unwrap().as_slice(),
        &["aarch64-darwin"]
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-020"))]
fn explicitly_selected_system_with_empty_checks_is_a_valid_static_only_run() {
    let (root, cache) = fixture(true);
    let backend = FakeNix::default();
    let mut args = request();
    args.systems = vec!["aarch64-linux".to_owned()];

    let (status, stdout, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    assert_eq!(
        backend.systems.lock().unwrap().as_slice(),
        &["aarch64-linux"]
    );
    assert_eq!(backend.evaluations.load(Ordering::SeqCst), 1);
    let result: Value = serde_json::from_slice(&stdout).expect("summary JSON");
    assert_eq!(result["counts"]["passed"], 3);
    assert_eq!(result["scope"]["systems"][0], "aarch64-linux");

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-011"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-020"))]
fn real_nix_distinguishes_missing_system_outputs_from_empty_check_sets() {
    if !nix_is_available() {
        eprintln!("skipping real-Nix selected-system test: Nix store is unavailable");
        return;
    }

    let (root, cache) = fixture(true);
    let backend = NixCli::default();
    let system = backend.host_system(&root).expect("host Nix system");
    fs::write(
        root.join("flake.nix"),
        format!(
            r#"{{
  description = "selected-system fixture";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = {{ self, nixpkgs }}: let
    pkgs = nixpkgs.legacyPackages."{system}";
    formatter = pkgs.writeShellScriptBin "noop-formatter" "exit 0";
  in {{
    formatter."{system}" = formatter;
    checks.aarch64-linux = {{}};
  }};
}}
"#
        ),
    )
    .expect("write fixture flake");
    lock_fixture_flake(&root);

    let mut missing = request();
    missing.systems = vec!["aarch64-darwin".to_owned()];
    let (status, stdout, _) = invoke(missing, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::from(2));
    let error: Value = serde_json::from_slice(&stdout).expect("missing-system error");
    assert_eq!(error["error"]["code"], "OperationalError");

    let mut empty = request();
    empty.systems = vec!["aarch64-linux".to_owned()];
    let (status, stdout, _) = invoke(empty, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let result: Value = serde_json::from_slice(&stdout).expect("empty-system summary");
    assert_eq!(result["counts"]["passed"], 3);
    assert_eq!(result["scope"]["systems"][0], "aarch64-linux");

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-005"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-012"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-015"))]
#[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-008"))]
#[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-009"))]
fn unmatched_selectors_bad_scopes_and_execution_options_are_usage_errors() {
    let mut args = request();
    args.selectors = vec!["unknown:*".to_owned()];
    assert!(validate_args(&args).is_ok());
    let list = CheckOperation::List(ListArgs {
        selectors: Vec::new(),
        systems: Vec::new(),
        offset: 0,
        limit: 0,
    });
    let invalid = CheckArgs {
        operation: Some(list),
        ..CheckArgs::default()
    };
    assert!(validate_args(&invalid).is_err());
    let retrieval_with_execution_flag = CheckArgs {
        operation: Some(CheckOperation::Failures(FailureArgs {
            run: None,
            offset: 0,
            limit: 20,
        })),
        jobs: Some(2),
        ..CheckArgs::default()
    };
    assert!(validate_args(&retrieval_with_execution_flag).is_err());
}

#[test]
#[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-009"))]
fn zero_limits_return_a_usage_error_from_the_command_entrypoint() {
    let (root, cache) = fixture(true);
    let mut args = request();
    args.operation = Some(CheckOperation::List(ListArgs {
        selectors: Vec::new(),
        systems: Vec::new(),
        offset: 0,
        limit: 0,
    }));
    let (status, stdout, _) = invoke(args, &root, &cache, &FakeNix::default(), true);
    assert_eq!(status, std::process::ExitCode::from(2));
    let error: Value = serde_json::from_slice(&stdout).expect("usage error");
    assert_eq!(error["error"]["code"], "UsageError");
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-CONTRACT-001"))]
fn check_fails_when_workspace_structure_is_invalid() {
    let (root, cache) = fixture(true);
    fs::remove_file(root.join(".bloomery/specs/CLI/CHECK/README.md"))
        .expect("remove feature README");
    let mut args = request();
    args.selectors = vec!["static:*".to_owned()];
    let backend = FakeNix::default();
    let (status, stdout, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    assert!(
        String::from_utf8(stdout)
            .expect("summary")
            .contains("MissingDocument")
    );
    assert_eq!(backend.formatter_runs.load(Ordering::SeqCst), 1);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-CONTRACT-002"))]
fn check_fails_when_automated_traceability_is_invalid() {
    let (root, cache) = fixture(true);
    let requirement = root.join(".bloomery/specs/CLI/CHECK/requirements/CONTRACT.toml");
    let contents = fs::read_to_string(&requirement).expect("fixture requirement");
    fs::write(
        &requirement,
        contents.replace("manual = true", "manual = false"),
    )
    .expect("automated requirement without evidence");
    let mut args = request();
    args.selectors = vec!["static:*".to_owned()];
    let backend = FakeNix::default();
    let (status, stdout, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    assert!(
        String::from_utf8(stdout)
            .expect("summary")
            .contains("MissingAutomatedTest")
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}
