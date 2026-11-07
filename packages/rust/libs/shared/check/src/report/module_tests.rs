use super::*;
use bloomery_workspace::source::MemoryTree;
use std::collections::BTreeMap;

fn files(root: &str) -> BTreeMap<String, String> {
    let mut files = target_tests::files();
    files.insert("tests/a.rs".into(), root.into());
    files.insert(
        "tests/child.rs".into(),
        "#[cfg_attr(any(), bloomery(\"DEMO-TESTS-BIND-001\"))] #[test] fn child() {}".into(),
    );
    files
}

fn analyze(files: BTreeMap<String, String>, features: Option<&[&str]>) -> StaticReport {
    analyze_tree_with_cfg(
        &MemoryTree::new(files).unwrap(),
        target_tests::config(),
        target_tests::context(features),
    )
}

fn child(report: &StaticReport) -> &TestRecord {
    report
        .tests
        .iter()
        .find(|test| test.name.as_deref() == Some("child"))
        .unwrap()
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-TARGET-005"))]
fn root_and_declaration_gates_apply_to_direct_children_and_routes() {
    let root = "#![cfg(feature=\"root\")]\n#[cfg(feature=\"edge\")] mod child;";
    for (features, expected) in [
        (
            Some(vec!["fast", "slow", "root", "edge"]),
            ConditionState::Satisfied,
        ),
        (Some(vec!["fast", "slow", "root"]), ConditionState::Rejected),
        (Some(vec!["fast", "slow", "edge"]), ConditionState::Rejected),
        (Some(vec!["root", "edge"]), ConditionState::Rejected),
        (None, ConditionState::Unresolved),
    ] {
        let report = analyze(files(root), features.as_deref());
        assert_eq!(child(&report).rust_target.as_ref().unwrap().state, expected);
        assert_eq!(
            report.requirements[0].coverage,
            if expected == ConditionState::Satisfied {
                Coverage::Bound
            } else {
                Coverage::Unresolved
            }
        );
        assert_eq!(report.completeness.relations, StageState::Partial);
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    }
    let root = "#[cfg(feature=\"first\")] #[path=\"child.rs\"] mod one;\n#[cfg(feature=\"second\")] #[path=\"child.rs\"] mod two;";
    let report = analyze(files(root), Some(&["fast", "slow", "second"]));
    assert_eq!(
        child(&report).rust_target.as_ref().unwrap().state,
        ConditionState::Satisfied
    );
    assert_eq!(report.cargo_test.as_ref().unwrap().modules.len(), 2);
    let mut local = files("mod child;");
    local
        .get_mut("tests/child.rs")
        .unwrap()
        .insert_str(0, "#[cfg(any())] ");
    let local = analyze(local, Some(&["fast", "slow"]));
    assert_eq!(
        child(&local).rust_target.as_ref().unwrap().state,
        ConditionState::Satisfied
    );
    assert_eq!(
        child(&local).rust_cfg.as_ref().unwrap().state,
        ConditionState::Rejected
    );
    assert_eq!(local.requirements[0].coverage, Coverage::Unresolved);
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-TARGET-006"))]
fn unresolved_module_paths_never_credit_coverage_and_rejected_gates_suppress_findings() {
    for (root, reason) in [
        ("mod absent;", "missing_module_path"),
        (
            "#[path=\"../../escape.rs\"] mod child;",
            "entrypoint escapes the repository",
        ),
        (
            "#[path=\"a.rs\"] mod child;",
            "self_referential_module_path",
        ),
        ("#[path=123] mod child;", "unsupported_module_path"),
        (
            "#[path=\"child.rs\"] #[path=\"child.rs\"] mod child;",
            "unsupported_module_path",
        ),
        (
            "#[cfg_attr(test, path=\"child.rs\")] mod child;",
            "unsupported_module_path",
        ),
    ] {
        let report = analyze(files(root), Some(&["fast", "slow"]));
        let module = &report.cargo_test.as_ref().unwrap().modules[0];
        assert_eq!(module.condition.state, ConditionState::Unresolved);
        assert!(module.condition.unresolved.contains(reason), "{:?}", module);
        assert_eq!(report.completeness.rust_modules, StageState::Partial);
        let diagnostic = report
            .diagnostics
            .iter()
            .find(|finding| finding.code == "RustModuleResolutionError")
            .unwrap();
        assert_eq!(diagnostic.location.as_ref(), Some(&module.location));
        assert_eq!(report.requirements[0].coverage, Coverage::Unresolved);
    }
    let mut ambiguous = files("mod child;");
    ambiguous.insert("tests/child/mod.rs".into(), "".into());
    let report = analyze(ambiguous, Some(&["fast", "slow"]));
    assert!(
        report.cargo_test.unwrap().modules[0]
            .condition
            .unresolved
            .contains("ambiguous_module_path")
    );
    let rejected = analyze(
        files("#[cfg(any())] #[path=\"../../escape.rs\"] mod child;"),
        Some(&["fast", "slow"]),
    );
    assert!(rejected.diagnostics.is_empty());
    assert_eq!(rejected.completeness.rust_modules, StageState::Complete);
    assert_eq!(
        rejected.cargo_test.unwrap().modules[0].condition.state,
        ConditionState::Rejected
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-013"))]
fn module_records_bind_inputs_but_never_infer_transitive_membership() {
    let mut sources = files("mod child;");
    let body = sources.remove("tests/child.rs").unwrap();
    sources.insert("tests/child/mod.rs".into(), format!("mod nested; {body}"));
    sources.insert(
        "tests/child/nested.rs".into(),
        "#[test] fn nested() {}".into(),
    );
    let first = analyze(sources.clone(), Some(&["fast", "slow"]));
    let target = first.cargo_test.as_ref().unwrap();
    assert_eq!(
        target.modules[0].path.as_deref(),
        Some("tests/child/mod.rs")
    );
    let nested = first
        .tests
        .iter()
        .find(|test| test.name.as_deref() == Some("nested"))
        .unwrap();
    assert!(
        nested
            .rust_target
            .as_ref()
            .unwrap()
            .unresolved
            .contains("rust_module_membership")
    );
    assert_eq!(
        first,
        analyze(
            sources.clone().into_iter().rev().collect(),
            Some(&["slow", "fast"])
        )
    );
    sources.insert("tests/a.rs".into(), "#[cfg(any())] mod child;".into());
    let changed = analyze(sources, Some(&["fast", "slow"]));
    assert_eq!(first.inputs.scope, changed.inputs.scope);
    assert_ne!(first.inputs.tree, changed.inputs.tree);
    assert_ne!(first.identity, changed.identity);
    assert_eq!(
        child(&changed).rust_target.as_ref().unwrap().state,
        ConditionState::Rejected
    );
    let mut external = files("unused");
    external.remove("tests/a.rs");
    external.insert(
        "outside/root.rs".into(),
        "#[path=\"../tests/child.rs\"] mod child; #[test] fn unselected() {}".into(),
    );
    external
        .get_mut("Cargo.toml")
        .unwrap()
        .replace_range(.., "[[test]]\nname='sample'\npath='outside/root.rs'\n");
    let report = analyze(external, Some(&["fast", "slow"]));
    assert_eq!(report.requirements[0].coverage, Coverage::Bound);
    assert!(
        !report
            .tests
            .iter()
            .any(|test| test.name.as_deref() == Some("unselected"))
    );
    let mut failed = files("fn broken( {");
    failed
        .get_mut("Cargo.toml")
        .unwrap()
        .replace_range(.., "[[test]]\nname='sample'\npath='tests/a.rs'\n");
    let report = analyze(failed, Some(&[]));
    assert_eq!(report.completeness.rust_modules, StageState::Failed);
    assert!(report.tests.is_empty());
}
