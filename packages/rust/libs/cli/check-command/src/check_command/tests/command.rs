use crate::check_command::command::{
    CheckArgs, CheckOperation, FailureArgs, ListArgs, discover_for_selection, validate_args,
};
use crate::check_command::execution::validate_execution_selection;
use crate::check_command::test_support::{
    FakeNix, fixture, fixture_without_specs, invoke, request,
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
    assert_eq!(result["result"]["counts"]["passed"], 3);
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
    assert_eq!(backend.systems.lock().unwrap().len(), 4);
    // Write mode discovers IDs before formatting and evaluates paths after it.
    assert_eq!(backend.evaluations.load(Ordering::SeqCst), 2);
    let result: Value = serde_json::from_slice(&stdout).expect("partial run JSON");
    assert_eq!(result["result"]["scope"]["checks"][0], "nix:*:core:test");
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
    let _ = fs::remove_dir_all(cache2);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-024"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-025"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-026"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-029"))]
fn missing_check_control_warnings_respect_explicit_values_and_available_groups() {
    let config: toml::Value = toml::from_str(
        "[checks]\nincludePackageChecks = false\n\n[checks.codeHealth]\nenable = false\n",
    )
    .expect("TOML");
    let missing = crate::check_command::command::missing_check_controls(&config, false);
    let keys = missing
        .iter()
        .map(|(key, _)| key.as_str())
        .collect::<Vec<_>>();
    assert!(keys.contains(&"checks.enable"));
    assert!(keys.contains(&"checks.crateCheck"));
    assert!(keys.contains(&"checks.clippySuppressions.enable"));
    assert!(!keys.contains(&"checks.includePackageChecks"));
    assert!(!keys.contains(&"checks.codeHealth.enable"));
    assert!(!keys.contains(&"checks.specifications.enable"));

    let disabled: toml::Value = toml::from_str("[checks]\nenable = false\n").expect("TOML");
    let missing = crate::check_command::command::missing_check_controls(&disabled, true);
    let keys = missing
        .iter()
        .map(|(key, _)| key.as_str())
        .collect::<Vec<_>>();
    assert!(!keys.contains(&"checks.crateCheck"));
    assert!(!keys.contains(&"checks.includePackageChecks"));
    assert!(!keys.contains(&"checks.workspaceDependencies"));
    assert!(keys.contains(&"checks.codeHealth.enable"));
    assert!(keys.contains(&"checks.clippySuppressions.enable"));
    assert!(keys.contains(&"checks.specifications.enable"));
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-027"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-028"))]
fn specification_checks_require_their_explicit_enable_control() {
    let config = "[specs]\nenable = true\n";
    let (root, cache) = fixture_without_specs(true, config);
    let backend = FakeNix::default();
    let selectors = vec!["static:*".to_owned()];
    let catalog = discover_for_selection(&root, &selectors, &[], &backend)
        .expect("catalog should contain no disabled spec checks");
    assert!(catalog.ids.is_empty());
    let mut args = request();
    args.selectors = vec!["static:structure".to_owned()];
    let (status, _, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::from(2));
    assert_eq!(backend.formatter_runs.load(Ordering::SeqCst), 0);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-SUPPRESSIONS-001"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-SUPPRESSIONS-002"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-SUPPRESSIONS-003"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-SUPPRESSIONS-005"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-SUPPRESSIONS-006"))]
fn clippy_suppression_check_scans_each_cargo_target_kind() {
    let config =
        "[specs]\nenable = false\n\n[checks.clippySuppressions]\nenable = true\nallow = []\n";
    let (root, cache) = fixture_without_specs(true, config);
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"suppression-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\nbuild = \"build.rs\"\n\n[lib]\npath = \"src/lib.rs\"\n\n[[bin]]\nname = \"fixture-bin\"\npath = \"src/main.rs\"\n\n[[example]]\nname = \"fixture-example\"\npath = \"examples/example.rs\"\n\n[[test]]\nname = \"fixture-test\"\npath = \"tests/integration.rs\"\n\n[[bench]]\nname = \"fixture-bench\"\npath = \"benches/bench.rs\"\n",
    )
    .expect("manifest");
    let targets = [
        "src/lib.rs",
        "src/main.rs",
        "build.rs",
        "examples/example.rs",
        "tests/integration.rs",
        "benches/bench.rs",
    ];
    for target in targets {
        let path = root.join(target);
        fs::create_dir_all(path.parent().expect("target parent")).expect("target directory");
        fs::write(path, "#![allow(clippy::all)]\npub fn fixture() {}\n").expect("target source");
    }

    let backend = FakeNix::default();
    let mut args = request();
    args.selectors = vec!["static:clippy-suppressions".to_owned()];
    let (status, stdout, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::FAILURE);
    let output = String::from_utf8_lossy(&stdout);
    for target in targets {
        assert!(
            output.contains(target),
            "missing diagnostic for {target}: {output}"
        );
    }
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-001"))]
#[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-001"))]
#[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-002"))]
#[cfg_attr(any(), bloomery("PARSER-SCANNING-HEALTH-006"))]
fn health_catalog_enablement_ignores_specs_scanners_build_toggle_and_system_count() {
    let config = "[specs]\nenable = false\n\n[scanners.rust]\nenabled = false\npaths = []\n\n[checks]\nenable = false\n\n[checks.codeHealth]\nenable = true\n";
    let (root, cache) = fixture_without_specs(true, config);
    let backend = FakeNix {
        names: vec!["base".to_owned()],
        ..FakeNix::default()
    };
    let selectors = vec!["static:*".to_owned()];
    let catalog =
        discover_for_selection(&root, &selectors, &[], &backend).expect("static-only catalog");
    assert_eq!(catalog.ids, ["static:code-health"]);
    let catalog = discover_for_selection(
        &root,
        &[],
        &["aarch64-linux".to_owned(), "x86_64-linux".to_owned()],
        &backend,
    )
    .expect("multi-system catalog");
    assert_eq!(
        catalog
            .ids
            .iter()
            .filter(|id| *id == "static:code-health")
            .count(),
        1
    );
    assert_eq!(backend.systems.lock().unwrap().len(), 2);

    fs::write(
        root.join(".bloomery/config.toml"),
        format!(
            "version = 1\n{}",
            config.replace("enable = true\n", "enable = false\n")
        ),
    )
    .expect("disable health");
    let catalog = discover_for_selection(&root, &selectors, &[], &backend)
        .expect("disabled static-only catalog");
    assert!(catalog.ids.is_empty());
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
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
        &["aarch64-linux", "aarch64-linux"]
    );
    assert_eq!(backend.evaluations.load(Ordering::SeqCst), 1);
    let result: Value = serde_json::from_slice(&stdout).expect("summary JSON");
    assert_eq!(result["result"]["counts"]["passed"], 3);
    assert_eq!(result["result"]["scope"]["systems"][0], "aarch64-linux");

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

#[test]
#[cfg_attr(any(), bloomery("CLI-BUILD-EXECUTION-024"))]
fn completed_check_runs_are_retained_for_retrieval() {
    let (root, cache) = fixture(true);
    let backend = FakeNix::default();
    let (status, _, _) = invoke(request(), &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    let store =
        crate::check_command::store::RunStore::open(&root, Some(&cache)).expect("run store");
    let (record, _) = store.latest().expect("retained run");
    assert!(!record.id.is_empty());
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-CONTRACT-004"))]
fn disabled_spec_management_omits_static_checks_from_the_catalog() {
    let (root, cache) = fixture_without_specs(true, "[specs]\nenable = false\n");
    let backend = FakeNix {
        names: vec!["core:test".to_owned()],
        ..FakeNix::default()
    };
    let catalog = discover_for_selection(&root, &[], &[], &backend).expect("catalog");
    assert!(
        !catalog.ids.iter().any(|id| id.starts_with("static:")),
        "static checks must be absent from the catalog when management is disabled"
    );
    assert!(
        catalog
            .ids
            .iter()
            .any(|id| id == "nix:x86_64-linux:core:test")
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-023"))]
fn disabled_spec_management_rejects_static_selectors() {
    let (root, cache) = fixture_without_specs(true, "[specs]\nenable = false\n");
    let backend = FakeNix::default();
    let mut args = request();
    args.selectors = vec!["static:structure".to_owned()];
    let (status, stdout, _) = invoke(args, &root, &cache, &backend, true);
    assert_eq!(status, std::process::ExitCode::from(2));
    let error: Value = serde_json::from_slice(&stdout).expect("usage error");
    assert_eq!(error["error"]["code"], "UsageError");
    assert_eq!(backend.formatter_runs.load(Ordering::SeqCst), 0);
    assert_eq!(backend.evaluations.load(Ordering::SeqCst), 0);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(cache);
}
