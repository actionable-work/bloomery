use super::*;
use bloomery_workspace::source::MemoryTree;
use std::collections::BTreeMap;

const MANIFEST: &str = "[[test]]\nname='sample'\npath='tests/a.rs'\nrequired-features=['slow', 'fast', 'fast']\n[[test]]\nname='other'\npath='tests/b.rs'\n";

pub(super) fn context(features: Option<&[&str]>) -> RustCfg {
    RustCfg {
        features: features.map(|items| items.iter().map(|item| (*item).into()).collect()),
        flags: [("test".into(), true)].into(),
        cargo_test: Some(CargoTestSelection {
            manifest: "Cargo.toml".into(),
            name: "sample".into(),
        }),
        ..RustCfg::default()
    }
}

pub(super) fn files() -> BTreeMap<String, String> {
    let mut files = tests::files();
    files.insert("Cargo.toml".into(), MANIFEST.into());
    files.insert(
        "tests/a.rs".into(),
        "#[cfg(test)] #[cfg_attr(any(), bloomery(\"DEMO-TESTS-BIND-001\"))] #[test] fn root() {}"
            .into(),
    );
    files.insert(
        "tests/b.rs".into(),
        "#[cfg_attr(any(), bloomery(\"DEMO-TESTS-BIND-002\"))] #[test] fn other() {}".into(),
    );
    files
        .get_mut(".bloomery/specs/DEMO/TESTS/requirements/BIND.toml")
        .unwrap()
        .push_str(&tests::record("DEMO-TESTS-BIND-002", false));
    files
}

pub(super) fn config() -> Config {
    let mut config = tests::config();
    config.scanners.playwright.enabled = false;
    config.scanners.nix.enabled = false;
    config
}

fn analyze(files: BTreeMap<String, String>, context: RustCfg) -> StaticReport {
    analyze_tree_with_cfg(&MemoryTree::new(files).unwrap(), config(), context)
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-TARGET-003"))]
fn required_features_and_harnesses_have_explicit_eligibility() {
    for (features, state, stage) in [
        (
            Some(vec!["fast", "slow"]),
            ConditionState::Satisfied,
            StageState::Complete,
        ),
        (
            Some(vec!["fast"]),
            ConditionState::Rejected,
            StageState::Complete,
        ),
        (Some(vec![]), ConditionState::Rejected, StageState::Complete),
        (None, ConditionState::Unresolved, StageState::Partial),
    ] {
        let report = analyze(files(), context(features.as_deref()));
        let target = report.cargo_test.as_ref().unwrap();
        assert_eq!(target.condition.state, state);
        assert_eq!(report.completeness.rust_target, stage);
        assert_eq!(
            target.required_features,
            ["fast".into(), "slow".into()].into()
        );
        assert_eq!(
            report.tests[0].rust_target.as_ref().unwrap(),
            &target.condition
        );
    }
    let mut sources = files();
    sources
        .get_mut("Cargo.toml")
        .unwrap()
        .insert_str(9, "harness=false\n");
    let custom = analyze(sources, context(Some(&["fast", "slow"])));
    assert_eq!(
        custom.cargo_test.unwrap().condition.unresolved,
        ["custom_test_harness".into()].into()
    );
    assert_eq!(custom.requirements[0].coverage, Coverage::Unresolved);
    let mut no_requirements = files();
    let manifest = MANIFEST.replace("required-features=['slow', 'fast', 'fast']\n", "");
    no_requirements.insert("Cargo.toml".into(), manifest);
    assert_eq!(
        analyze(no_requirements, context(None))
            .cargo_test
            .unwrap()
            .condition
            .state,
        ConditionState::Satisfied
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-TARGET-004"))]
fn coverage_requires_local_cfg_and_entrypoint_eligibility() {
    let report = analyze(files(), context(Some(&["fast", "slow"])));
    assert_eq!(report.requirements[0].coverage, Coverage::Bound);
    assert_eq!(report.requirements[1].coverage, Coverage::Unresolved);
    assert_eq!(
        report.tests[1].rust_target.as_ref().unwrap().unresolved,
        ["rust_module_membership".into()].into()
    );
    assert_eq!(report.completeness.relations, StageState::Partial);
    let mut cfg = context(Some(&["fast", "slow"]));
    cfg.flags.insert("test".into(), false);
    let rejected = analyze(files(), cfg);
    assert_eq!(
        rejected.cargo_test.unwrap().condition.state,
        ConditionState::Satisfied
    );
    assert_eq!(rejected.requirements[0].coverage, Coverage::Unresolved);
    let mut cfg = context(None);
    cfg.cargo_test.as_mut().unwrap().name = "other".into();
    let other = analyze(files(), cfg);
    assert_eq!(other.requirements[0].coverage, Coverage::Unresolved);
    assert_eq!(other.requirements[1].coverage, Coverage::Bound);
    assert!(
        !other
            .diagnostics
            .iter()
            .any(|finding| finding.code == "MissingAutomatedTest")
    );

    for manifest in [
        "invalid [",
        "[[test]]\nname='sample'",
        "[[test]]\nname='sample'\npath='missing.rs'",
    ] {
        let mut sources = files();
        sources.insert("Cargo.toml".into(), manifest.into());
        let report = analyze(sources, context(Some(&["fast", "slow"])));
        assert_eq!(report.completeness.rust_target, StageState::Failed);
        assert_eq!(report.completeness.rust_cfg, StageState::Complete);
        assert_eq!(report.tests.len(), 2);
        assert!(
            report
                .requirements
                .iter()
                .all(|requirement| requirement.coverage == Coverage::Unresolved)
        );
        assert!(report.bindings.iter().all(
            |binding| binding.rust_target.as_ref().unwrap().state == ConditionState::Unresolved
        ));
    }
    let tree = MemoryTree::new(files()).unwrap();
    let mut disabled = config();
    disabled.scanners.rust.enabled = false;
    let report = analyze_tree_with_cfg(&tree, disabled, context(None));
    assert_eq!(report.completeness.rust_target, StageState::Disabled);
    assert_eq!(report.completeness.rust_modules, StageState::Disabled);
    assert!(report.cargo_test.is_none());
    assert!(report.diagnostics.is_empty());
    assert_eq!(
        analyze_tree(&tree, config()).completeness.rust_target,
        StageState::NotAttempted
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-012"))]
fn target_inputs_are_bound_and_json_selectors_are_strict() {
    let first = analyze(files(), context(Some(&["fast", "slow"])));
    let canonical = RustCfg::parse_json(r#"{"flags":{"test":true},"features":["slow","fast","fast"],"cargo_test":{"name":"sample","manifest":"Cargo.toml"}}"#).unwrap();
    assert_eq!(first, analyze(files(), canonical));
    let mut cfg = context(Some(&["fast", "slow"]));
    cfg.cargo_test.as_mut().unwrap().name = "other".into();
    let second = analyze(files(), cfg);
    assert_eq!(first.inputs.tree, second.inputs.tree);
    assert_ne!(first.inputs.scope, second.inputs.scope);
    assert_ne!(first.identity, second.identity);
    let mut sources = files();
    sources.insert(
        "Cargo.toml".into(),
        MANIFEST.replace("['slow', 'fast', 'fast']", "['fast']"),
    );
    let changed = analyze(sources, context(Some(&["fast", "slow"])));
    assert_eq!(first.inputs.scope, changed.inputs.scope);
    assert_ne!(first.inputs.tree, changed.inputs.tree);
    assert_ne!(first.identity, changed.identity);
    for invalid in [
        r#"{"cargo_test":[]}"#,
        r#"{"cargo_test":["Cargo.toml","sample"]}"#,
        r#"{"cargo_test":{}}"#,
        r#"{"cargo_test":{"manifest":"../Cargo.toml","name":"sample"}}"#,
        r#"{"cargo_test":{"manifest":"Cargo.toml","name":"sample","extra":true}}"#,
        r#"{"cargo_test":{"manifest":1,"name":"sample"}}"#,
    ] {
        assert!(RustCfg::parse_json(invalid).is_err(), "{invalid}");
    }
    let mut invalid = context(None);
    invalid.cargo_test.as_mut().unwrap().name.clear();
    let report = analyze(files(), invalid);
    assert_eq!(report.completeness.rust_cfg, StageState::Failed);
    assert_eq!(report.completeness.tree, StageState::NotAttempted);
}
