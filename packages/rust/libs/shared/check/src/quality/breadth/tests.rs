use crate::quality::tests::{analyze, files, tagged};
use std::collections::BTreeMap;

fn linked_files(count: usize) -> BTreeMap<String, String> {
    let mut files = files("");
    let mut ids = vec![
        "DEMO-TESTS-RULE-001".to_owned(),
        "DEMO-TESTS-RULE-002".to_owned(),
    ];
    let group = files
        .get_mut(".bloomery/specs/DEMO/TESTS/requirements/RULE.toml")
        .unwrap();
    for index in 4..count + 2 {
        let id = format!("DEMO-TESTS-RULE-{index:03}");
        group.push_str(&format!("\n[[requirements]]\nid = '{id}'\ntitle = 'One behavior'\nmanual = false\nears = {{type = 'ubiquitous', system = 'example', action = 'work'}}\n"));
        ids.push(id);
    }
    let references: Vec<_> = ids.iter().map(String::as_str).collect();
    files.insert("tests/a.rs".into(), tagged("broad", &references));
    files
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-BREADTH-001"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-BREADTH-002"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-BREADTH-003"))]
fn breadth_enforces_the_boundary_and_deduplicates_valid_ids() {
    let mut files = linked_files(6);
    files.get_mut("tests/a.rs").unwrap().insert_str(0, "#[cfg_attr(any(), bloomery(\"DEMO-TESTS-RULE-001\"), bloomery(\"DEMO-TESTS-RULE-003\"), bloomery(\"DEMO-TESTS-RULE-999\"))]\n");
    let (_, context, mut output, assessment) = analyze(files.clone(), "", true);
    assert_eq!(assessment.report.tests.len(), 1);
    assert_eq!(assessment.report.tests[0].requirement_ids.len(), 6);
    assert_eq!(assessment.diagnostics.len(), 1);
    assert_eq!(assessment.diagnostics[0].code, "TooManyRequirementsForTest");
    output.tests.extend(output.tests.clone());
    let duplicated = crate::quality::assess_links(&context, &output, true).unwrap();
    assert_eq!(duplicated.report.tests.len(), 1);
    let errors = crate::validate_evidence(&context, &output).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|finding| finding.code == "OrphanTestReference")
    );
    assert!(
        errors
            .iter()
            .any(|finding| finding.code == "IllegalManualAutomation")
    );
    let (_, _, _, equal) = analyze(files, "maxRequirementsPerTest = 6", true);
    assert!(equal.diagnostics.is_empty());
    assert!(analyze(linked_files(2), "", true).3.diagnostics.is_empty());
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-BREADTH-004"))]
fn breadth_counts_direct_sites_and_metadata_files_independently() {
    let mut files = linked_files(2);
    files.get_mut("tests/a.rs").unwrap().push_str(&format!(
        "mod nested {{ {} }}",
        tagged("broad", &["DEMO-TESTS-RULE-001"])
    ));
    files.insert("e2e/a.ts".into(), "test.describe('suite', {tag: ['@bloomery:DEMO-TESTS-RULE-001', '@bloomery:DEMO-TESTS-RULE-002']}, () => {test('child', () => {});});".into());
    files.insert("checks/generated.nix".into(), "{ a.passthru.bloomery = ['DEMO-TESTS-RULE-001']; b.passthru.bloomery = ['DEMO-TESTS-RULE-002']; }".replace('\'', "\""));
    let (_, _, _, assessment) = analyze(files, "maxRequirementsPerTest = 1", true);
    assert_eq!(assessment.report.tests.len(), 4);
    assert_eq!(assessment.diagnostics.len(), 3);
    assert!(
        assessment
            .diagnostics
            .iter()
            .all(|finding| finding.code == "TooManyRequirementsForTest")
    );
    assert_eq!(
        assessment
            .report
            .tests
            .iter()
            .filter(|row| row.site.scanner == "nix")
            .count(),
        1
    );
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-BREADTH-005"))]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-BREADTH-001"))]
fn named_exceptions_precede_file_overrides_and_remain_visible() {
    let policy = "[checks.specifications.testExceptions.a_file]\npath = 'tests/**/*.rs'\nmaxRequirements = 7\nreason = 'Whole-file integration contract'\n[checks.specifications.testExceptions.z_named]\npath = 'tests/a.rs'\nname = 'broad'\nmaxRequirements = 6\nreason = 'Named protocol roundtrip'";
    let (tree, context, _, assessment) = analyze(linked_files(6), policy, true);
    assert!(assessment.diagnostics.is_empty());
    let row = &assessment.report.tests[0];
    assert_eq!(row.limit, 6);
    assert_eq!(
        row.exception_reason.as_deref(),
        Some("Named protocol roundtrip")
    );
    let mut reordered = linked_files(6);
    let source = reordered.get_mut("tests/a.rs").unwrap();
    *source = source.replace("fn broad", "fn other");
    let (_, _, _, fallback) = analyze(reordered, policy, true);
    assert_eq!(fallback.report.tests[0].limit, 7);
    let lowered = policy.replace("maxRequirements = 6", "maxRequirements = 5");
    assert!(
        analyze(linked_files(6), &lowered, true).3.diagnostics[0]
            .notes
            .iter()
            .any(|note| note.contains("Named protocol roundtrip"))
    );
    let report = crate::report::analyze_tree(&tree, context.config);
    assert_eq!(report.version, 10);
    assert_eq!(
        report.quality.unwrap().tests[0].requirement_ids,
        row.requirement_ids
    );
    assert!(row.requirement_ids.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-BREADTH-006"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-BREADTH-007"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-BREADTH-008"))]
fn partial_and_ignored_tests_preserve_breadth_findings_and_declarations() {
    let mut files = linked_files(6);
    files
        .get_mut("tests/a.rs")
        .unwrap()
        .insert_str(0, "#[ignore] #[cfg(any())]\n");
    let (tree, context, _, assessment) = analyze(files, "", false);
    let finding = &assessment.diagnostics[0];
    assert_eq!(finding.code, "TooManyRequirementsForTest");
    assert!(!assessment.report.discovery_complete);
    assert!(
        finding
            .message
            .contains("6 distinct automated requirements; limit is 5")
    );
    assert_eq!(
        finding.location.as_ref().unwrap().path.to_str(),
        Some("tests/a.rs")
    );
    assert_eq!(
        finding
            .notes
            .iter()
            .filter(|note| note
                .contains("declared at .bloomery/specs/DEMO/TESTS/requirements/RULE.toml:"))
            .count(),
        6
    );
    let scoped = crate::report::analyze_tree_with_cfg(
        &tree,
        context.config,
        crate::report::RustCfg::default(),
    );
    assert_eq!(
        scoped.completeness.relations,
        crate::report::StageState::Partial
    );
    assert!(
        scoped
            .diagnostics
            .iter()
            .any(|finding| finding.code == "TooManyRequirementsForTest")
    );
}
