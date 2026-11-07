use super::*;
use bloomery_workspace::source::MemoryTree;
use std::collections::BTreeMap;

const ID: &str = "DEMO-TESTS-BIND-001";
const TEST: &str = "#[cfg_attr(any(), bloomery(\"DEMO-TESTS-BIND-001\"))] #[test] fn tied() {}";

fn files(source: &str) -> BTreeMap<String, String> {
    let mut files = tests::files();
    files.insert("tests/a.rs".into(), source.into());
    files.insert("e2e/a.ts".into(), "test('unbound', async () => {});".into());
    files.insert("checks/a.test.nix".into(), "{}".into());
    files
}

fn analyze(source: &str, cfg: Option<RustCfg>) -> StaticReport {
    let tree = MemoryTree::new(files(source)).unwrap();
    match cfg {
        Some(cfg) => analyze_tree_with_cfg(&tree, tests::config(), cfg),
        None => analyze_tree(&tree, tests::config()),
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-014"))]
fn ignore_never_credits_coverage_but_compilation_and_literals_stay_independent() {
    for attribute in [
        "#[ignore]",
        "#[ignore = \"pending\"]",
        "#[cfg_attr(test, ignore)]",
    ] {
        for cfg in [
            None,
            Some(RustCfg::parse_json(r#"{"flags":{"test":true}}"#).unwrap()),
        ] {
            let report = analyze(&format!("{attribute}\n{TEST}"), cfg.clone());
            assert!(report.diagnostics.is_empty());
            assert_eq!(report.bindings.len(), 1);
            assert_eq!(report.bindings[0].resolution, BindingResolution::Declared);
            assert_eq!(report.requirements[0].coverage, Coverage::Unresolved);
            let harness = report.bindings[0].rust_harness.as_ref().unwrap();
            assert_eq!(harness.state, ConditionState::Unresolved);
            assert!(harness.unresolved.contains("ignored_test_attribute"));
            let test = report
                .tests
                .iter()
                .find(|test| test.scanner == "rust")
                .unwrap();
            assert_eq!(test.rust_harness.as_ref(), Some(harness));
            assert_eq!(report.completeness.rust_harness, StageState::Partial);
            assert_eq!(report.completeness.discovery, StageState::Complete);
            assert_eq!(report.completeness.relations, StageState::Partial);
            if cfg.is_some() {
                assert_eq!(report.completeness.rust_cfg, StageState::Complete);
                assert_eq!(
                    report.bindings[0].rust_cfg.as_ref().unwrap().state,
                    ConditionState::Satisfied
                );
            } else {
                assert!(report.bindings[0].rust_cfg.is_none());
                assert_eq!(report.completeness.rust_cfg, StageState::NotAttempted);
            }
        }
    }
    let another = format!("#[ignore] {TEST}\n{}", TEST.replace("tied", "eligible"));
    assert_eq!(
        analyze(&another, None).requirements[0].coverage,
        Coverage::Bound
    );
    for (manual, code) in [
        (false, "OrphanTestReference"),
        (true, "IllegalManualAutomation"),
    ] {
        let mut sources = files(&format!("#[ignore] {TEST}"));
        let group = sources
            .get_mut(".bloomery/specs/DEMO/TESTS/requirements/BIND.toml")
            .unwrap();
        *group = if manual {
            group.replace("manual = false", "manual = true")
        } else {
            group.replace(ID, "DEMO-TESTS-BIND-002")
        };
        let report = analyze_tree(&MemoryTree::new(sources).unwrap(), tests::config());
        let finding = report
            .diagnostics
            .iter()
            .find(|finding| finding.code == code)
            .unwrap();
        assert_eq!(
            finding.location.as_ref(),
            Some(&report.bindings[0].location)
        );
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-014"))]
fn conditional_ignore_uses_only_explicit_facts_and_never_inherits_parent_ignore() {
    for (attribute, context, expected) in [
        (
            "#[cfg_attr(any(), ignore)]",
            None,
            ConditionState::Satisfied,
        ),
        (
            "#[cfg_attr(all(), ignore)]",
            None,
            ConditionState::Unresolved,
        ),
        (
            "#[cfg_attr(feature=\"fast\", ignore)]",
            None,
            ConditionState::Unresolved,
        ),
        (
            "#[cfg_attr(feature=\"fast\", ignore)]",
            Some(r#"{"features":[]}"#),
            ConditionState::Satisfied,
        ),
        (
            "#[cfg_attr(feature=\"fast\", ignore)]",
            Some(r#"{"features":["fast"]}"#),
            ConditionState::Unresolved,
        ),
        (
            "#[cfg_attr(test, cfg_attr(feature=\"fast\", ignore=\"later\"))]",
            Some(r#"{"flags":{"test":true},"features":[]}"#),
            ConditionState::Satisfied,
        ),
        (
            "#[cfg_attr(unknown, cfg_attr(any(), ignore))]",
            None,
            ConditionState::Satisfied,
        ),
        ("#[ignore=123]", None, ConditionState::Unresolved),
        ("#[ignore(123)]", None, ConditionState::Unresolved),
        ("#[cfg_attr(test)]", None, ConditionState::Unresolved),
        (
            "#[cfg_attr(unsupported(test), ignore)]",
            None,
            ConditionState::Unresolved,
        ),
        (
            "#[cfg_attr(not(test, windows), ignore)]",
            None,
            ConditionState::Unresolved,
        ),
    ] {
        let report = analyze(
            &format!("{attribute} {TEST}"),
            context.map(|json| RustCfg::parse_json(json).unwrap()),
        );
        assert_eq!(
            report.bindings[0].rust_harness.as_ref().unwrap().state,
            expected,
            "{attribute}"
        );
        assert_eq!(
            report.requirements[0].coverage,
            if expected == ConditionState::Satisfied {
                Coverage::Bound
            } else {
                Coverage::Unresolved
            }
        );
    }
    let report = analyze(
        &format!("#![ignore]\n#[cfg_attr(all(), ignore)] mod parent {{ {TEST} }}"),
        None,
    );
    assert_eq!(report.requirements[0].coverage, Coverage::Bound);
    // Conditional test candidates cannot become bindings just because ignore is absent.
    for predicate in ["all()", "any()", "test"] {
        let report = analyze(
            &TEST.replace("#[test]", &format!("#[cfg_attr({predicate}, test)]")),
            None,
        );
        assert_eq!(report.requirements[0].coverage, Coverage::Unresolved);
        assert!(
            report.bindings[0]
                .rust_harness
                .as_ref()
                .unwrap()
                .unresolved
                .contains("conditional_test_attribute")
        );
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-015"))]
fn harness_completeness_tracks_independent_failures_and_disabled_sources() {
    let tree = MemoryTree::new(files(&format!("#[ignore] {TEST}"))).unwrap();
    let mut config = tests::config();
    config.scanners.rust.enabled = false;
    let report = analyze_tree(&tree, config.clone());
    assert_eq!(report.completeness.rust_harness, StageState::Disabled);
    assert!(report.tests.iter().all(|test| test.rust_harness.is_none()));
    config.scanners.rust.enabled = true;
    config.scanners.rust.paths = vec!["missing/**/*.rs".into()];
    assert_eq!(
        analyze_tree(&tree, config).completeness.rust_harness,
        StageState::Partial
    );
    assert_eq!(
        analyze("fn broken( {", None).completeness.rust_harness,
        StageState::Failed
    );
    let mut config = tests::config();
    config.specs.dir = "../escape".into();
    assert_eq!(
        analyze_tree(&tree, config).completeness.rust_harness,
        StageState::NotAttempted
    );
    let mut sources = files(TEST);
    sources.insert("e2e/a.ts".into(), "const invalid = ;".into());
    let report = analyze_tree(&MemoryTree::new(sources).unwrap(), tests::config());
    assert_eq!(report.completeness.rust_harness, StageState::Failed);
    assert!(report.tests.is_empty());
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-015"))]
fn harness_identities_bind_source_and_caller_facts_deterministically() {
    let source = format!("#[cfg_attr(feature=\"fast\", ignore)] {TEST}");
    let a = RustCfg::parse_json(r#"{"features":["fast","fast"]}"#).unwrap();
    let first = analyze(&source, Some(a));
    let b = RustCfg::parse_json(r#"{"features":["fast"]}"#).unwrap();
    assert_eq!(first, analyze(&source, Some(b.clone())));
    let reversed = MemoryTree::new(files(&source).into_iter().rev()).unwrap();
    assert_eq!(first, analyze_tree_with_cfg(&reversed, tests::config(), b));
    let off = analyze(
        &source,
        Some(RustCfg::parse_json(r#"{"features":[]}"#).unwrap()),
    );
    assert_eq!(first.inputs.tree, off.inputs.tree);
    assert_ne!(first.inputs.scope, off.inputs.scope);
    assert_ne!(first.identity, off.identity);
    let changed = analyze(TEST, None);
    let ignored = analyze(&format!("#[ignore] {TEST}"), None);
    assert_ne!(changed.inputs.tree, ignored.inputs.tree);
    assert_eq!(changed.inputs.scope, ignored.inputs.scope);
    assert_ne!(changed.identity, ignored.identity);
    assert_eq!(first.version, 6);
}
