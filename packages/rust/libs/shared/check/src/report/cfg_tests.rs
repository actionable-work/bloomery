use super::*;
use bloomery_workspace::source::MemoryTree;

fn context(features: Option<&[&str]>) -> RustCfg {
    RustCfg {
        features: features.map(|features| features.iter().map(|value| (*value).into()).collect()),
        flags: [("test".into(), true), ("windows".into(), false)].into(),
        values: [("target_os".into(), ["linux".into()].into())].into(),
        ..RustCfg::default()
    }
}

fn scan(source: &str, cfg: RustCfg) -> StaticReport {
    let tree = MemoryTree::new([("tests/a.rs", source)]).expect("tree");
    let mut config = bloomery_model::Config::default();
    config.scanners.rust.paths = vec!["tests/**/*.rs".into()];
    analyze_tree_with_cfg(&tree, config, cfg)
}

fn state(source: &str, cfg: RustCfg) -> ConditionState {
    let report = scan(source, cfg);
    assert_eq!(report.tests.len(), 1, "{:?}", report.diagnostics);
    report.tests[0].rust_cfg.as_ref().expect("condition").state
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-SCANNING-CFG-001"))]
fn facts_are_explicit_and_features_distinguish_absent_from_unknown() {
    for (predicate, expected) in [
        ("test", ConditionState::Satisfied),
        ("windows", ConditionState::Rejected),
        ("target_os = \"linux\"", ConditionState::Satisfied),
        ("target_os = \"windows\"", ConditionState::Rejected),
        ("custom_flag", ConditionState::Unresolved),
    ] {
        assert_eq!(
            state(
                &format!("#[cfg({predicate})] #[test] fn sample() {{}}"),
                context(Some(&[]))
            ),
            expected
        );
    }
    let source = "#[cfg(feature = \"fast\")] #[test] fn sample() {}";
    assert_eq!(
        state(source, context(Some(&["fast"]))),
        ConditionState::Satisfied
    );
    assert_eq!(state(source, context(Some(&[]))), ConditionState::Rejected);
    assert_eq!(state(source, context(None)), ConditionState::Unresolved);
    let label_only = RustCfg {
        target: Some("x86_64-unknown-linux-gnu".into()),
        ..RustCfg::default()
    };
    assert_eq!(
        state(
            "#[cfg(target_os = \"linux\")] #[test] fn sample() {}",
            label_only
        ),
        ConditionState::Unresolved
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-SCANNING-CFG-002"))]
fn file_module_and_boolean_gates_preserve_inert_metadata() {
    for (predicate, expected) in [
        ("all()", ConditionState::Satisfied),
        ("any()", ConditionState::Rejected),
        ("all(any(), custom)", ConditionState::Rejected),
        ("any(all(), custom)", ConditionState::Satisfied),
        ("not(custom)", ConditionState::Unresolved),
        ("not(windows)", ConditionState::Satisfied),
    ] {
        assert_eq!(
            state(
                &format!("#[cfg({predicate})] #[test] fn sample() {{}}"),
                context(Some(&[]))
            ),
            expected
        );
    }
    let source = "#![cfg(test)]\n#[cfg_attr(feature=\"fast\", cfg(windows))]\nmod nested { #[cfg_attr(any(), bloomery(\"DEMO-TESTS-BIND-001\"))] #[test] fn sample() {} }";
    let report = scan(source, context(Some(&["fast"])));
    assert_eq!(
        report.tests[0].rust_cfg.as_ref().expect("condition").state,
        ConditionState::Rejected
    );
    assert_eq!(report.bindings.len(), 1);
    assert_eq!(report.bindings[0].rust_cfg, report.tests[0].rust_cfg);
    assert_eq!(state(source, context(Some(&[]))), ConditionState::Satisfied);
    let tree = MemoryTree::new([("tests/a.rs", source)]).expect("tree");
    let mut config = bloomery_model::Config::default();
    config.scanners.rust.paths = vec!["tests/**/*.rs".into()];
    assert!(analyze_tree(&tree, config).tests[0].rust_cfg.is_none());
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-SCANNING-CFG-003"))]
fn unsupported_and_malformed_conditions_never_become_success() {
    for source in [
        "#[cfg(unsupported(foo))] #[test] fn sample() {}",
        "#[cfg(any(all(), unsupported(foo)))] #[test] fn sample() {}",
        "#[cfg_attr(unsupported(foo), allow(dead_code))] #[test] fn sample() {}",
        "#[cfg_attr(123, allow(dead_code))] #[test] fn sample() {}",
        "#[cfg] #[test] fn sample() {}",
        "#[cfg_attr(test, cfg_attr(feature=\"fast\", test))] fn sample() {}",
        "#[cfg(not(test, windows))] #[test] fn sample() {}",
        "#[cfg(broken = 123)] #[test] fn sample() {}",
        "#[cfg_attr(feature=\"fast\", test)] fn sample() {}",
    ] {
        let report = scan(source, context(Some(&["fast"])));
        let condition = report.tests[0].rust_cfg.as_ref().expect("condition");
        assert_eq!(condition.state, ConditionState::Unresolved);
        assert!(!condition.unresolved.is_empty());
        assert_eq!(report.completeness.rust_cfg, StageState::Partial);
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-010"))]
fn scope_normalizes_sets_and_changes_identity_without_changing_tree() {
    let a = RustCfg::parse_json(r#"{"features":["fast","slow","fast"],"flags":{"test":true}}"#)
        .expect("context");
    let b = RustCfg::parse_json(r#"{"flags":{"test":true},"features":["slow","fast"]}"#)
        .expect("context");
    let source = "#[cfg(feature=\"fast\")] #[test] fn sample() {}";
    let first = scan(source, a);
    assert_eq!(first, scan(source, b));
    let second = scan(source, context(Some(&[])));
    assert_eq!(first.inputs.tree, second.inputs.tree);
    assert_eq!(first.inputs.configuration, second.inputs.configuration);
    assert_ne!(first.inputs.scope, second.inputs.scope);
    assert_ne!(first.identity, second.identity);
    assert_eq!(second.completeness.relations, StageState::NotAttempted); // no specs
    assert!(
        second
            .scope
            .limitations
            .iter()
            .any(|limit| limit == "module_graph")
    );
    for invalid in [
        "null",
        "[]",
        "{",
        r#"{"unknown":true}"#,
        r#"{"flags":{"test":1}}"#,
        r#"{"features":"fast"}"#,
        r#"{"values":{"target_os":[1]}}"#,
        r#"{"target":""}"#,
    ] {
        assert!(RustCfg::parse_json(invalid).is_err(), "{invalid}");
    }
}
