use super::*;
use bloomery_model::{Config, config::parse_contents};
use bloomery_scanning::scan_tree;
use bloomery_workspace::{load_tree, source::MemoryTree};

const ID: &str = "DEMO-TESTS-RULE-001";
const OTHER: &str = "DEMO-TESTS-RULE-002";
const MANUAL: &str = "DEMO-TESTS-RULE-003";

pub(super) fn files(rust: &str) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    files.insert(
        ".bloomery/specs/DEMO/TESTS/design/main.md".into(),
        "# Main\n".into(),
    );
    for (path, id) in [
        (".bloomery/specs/DEMO", "DEMO"),
        (".bloomery/specs/DEMO/TESTS", "TESTS"),
    ] {
        files.insert(format!("{path}/README.md"), format!("---\nid: {id}\nname: Example\ntagline: Example\ndescription: Example\n---\n# Example\n"));
    }
    let records = [ID, OTHER, MANUAL].iter().map(|id| format!("[[requirements]]\nid = \"{id}\"\ntitle = \"One behavior\"\nmanual = {}\nears = {{ type = \"ubiquitous\", system = \"example\", action = \"work\" }}\n", *id == MANUAL)).collect::<String>();
    files.insert(
        ".bloomery/specs/DEMO/TESTS/requirements/RULE.toml".into(),
        format!("group = \"RULE\"\n{records}"),
    );
    files.insert("tests/a.rs".into(), rust.into());
    files
}

pub(super) fn config(policy: &str) -> Config {
    parse_contents(&format!("version = 2\n[scanners.rust]\npaths = ['tests/**/*.rs']\n[scanners.playwright]\npaths = ['e2e/**/*.ts']\n[scanners.nix]\npaths = ['checks/**/*.nix']\ntestPaths = ['checks/**/*.test.nix']\n[checks.specifications]\n{policy}"), Path::new(".bloomery/config.toml")).expect("configuration").config
}

pub(super) fn tagged(name: &str, ids: &[&str]) -> String {
    let tags = ids
        .iter()
        .map(|id| format!("#[cfg_attr(any(), bloomery(\"{id}\"))]\n"))
        .collect::<String>();
    format!("{tags}#[test] fn {name}() {{ for _ in 0..20 {{}} }}\n")
}

pub(super) fn analyze(
    files: BTreeMap<String, String>,
    policy: &str,
    complete: bool,
) -> (MemoryTree, Context, ScanOutput, Assessment) {
    let tree = MemoryTree::new(files).expect("tree");
    let context = load_tree(&tree, config(policy)).expect("specifications");
    let output = scan_tree(&tree, &context.config.scanners)
        .expect("scan")
        .output;
    let assessment = assess(&context, &output, &tree, complete).expect("assessment");
    (tree, context, output, assessment)
}

fn count(assessment: &Assessment, id: &str) -> usize {
    assessment
        .report
        .requirements
        .iter()
        .find(|requirement| requirement.id == id)
        .expect("requirement")
        .sites
        .len()
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-GRANULARITY-001"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-GRANULARITY-002"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-GRANULARITY-004"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-GRANULARITY-005"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-GRANULARITY-006"))]
fn granularity_counts_declarations_and_preserves_the_threshold_boundary() {
    let rust = (0..6)
        .map(|index| format!("mod m{index} {{ {} }}", tagged("same_name", &[ID, ID])))
        .collect::<String>();
    let (_, _, _, assessment) = analyze(files(&rust), "", true);
    assert_eq!(count(&assessment, ID), 6);
    assert_eq!(assessment.diagnostics.len(), 1);
    assert_eq!(assessment.diagnostics[0].code, "TooManyTestsForRequirement");
    let (_, _, mut output, _) = analyze(files(&rust), "maxTestsPerRequirement = 6", true);
    output.tests.extend(output.tests.clone());
    let (_, context, _, _) = analyze(files(&rust), "maxTestsPerRequirement = 6", true);
    let equal = assess_links(&context, &output, true).unwrap();
    assert_eq!(count(&equal, ID), 6);
    assert!(equal.diagnostics.is_empty());
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-GRANULARITY-003"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-GRANULARITY-007"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-GRANULARITY-008"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-GRANULARITY-009"))]
fn counts_combine_scanners_without_expanding_suites_or_nix_files() {
    let mut files = files(&tagged("rust", &[ID, OTHER]));
    files.insert("e2e/a.ts".into(), format!("test.describe('suite', {{tag: ['@bloomery:{ID}', '@bloomery:{OTHER}']}}, () => {{ test('child', () => {{}}); }});"));
    files.insert("checks/generated.nix".into(), format!("{{ a.passthru.bloomery = [\"{ID}\" \"{ID}\" \"{OTHER}\"]; b.passthru.bloomery = [\"{ID}\"]; }}"));
    let (_, _, _, assessment) = analyze(files, "", true);
    assert_eq!(count(&assessment, ID), 3);
    assert_eq!(count(&assessment, OTHER), 3);
    assert!(assessment.diagnostics.is_empty());
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-GRANULARITY-010"))]
fn unknown_and_manual_references_keep_their_existing_diagnostics() {
    let rust = (0..7)
        .map(|index| tagged(&format!("case{index}"), &[MANUAL, "DEMO-TESTS-RULE-999"]))
        .collect::<String>();
    let (_, context, output, _) = analyze(files(&rust), "", true);
    let errors = crate::validate_evidence(&context, &output).unwrap_err();
    assert_eq!(
        errors
            .iter()
            .filter(|finding| finding.code == "IllegalManualAutomation")
            .count(),
        7
    );
    assert_eq!(
        errors
            .iter()
            .filter(|finding| finding.code == "OrphanTestReference")
            .count(),
        7
    );
    assert!(
        !errors
            .iter()
            .any(|finding| finding.code == "TooManyTestsForRequirement")
    );
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-GRANULARITY-011"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-GRANULARITY-012"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-QUALITY-010"))]
fn authored_links_ignore_execution_eligibility_and_allow_reasoned_overrides() {
    let rust = (0..7)
        .map(|index| {
            format!(
                "#[cfg(any())] #[ignore] {}",
                tagged(&format!("case{index}"), &[ID])
            )
        })
        .collect::<String>();
    let (_, _, _, assessment) = analyze(files(&rust), "", true);
    let finding = &assessment.diagnostics[0];
    assert!(finding.message.contains("7 distinct"));
    assert!(finding.message.contains("limit is 5"));
    assert!(
        finding
            .location
            .as_ref()
            .unwrap()
            .path
            .ends_with("RULE.toml")
    );
    assert_eq!(
        finding
            .notes
            .iter()
            .filter(|note| note.starts_with("rust tests/a.rs:"))
            .count(),
        7
    );
    let policy = format!(
        "[checks.specifications.requirementExceptions.'{ID}']\nmaxTests = 7\nreason = 'Independent boundary cases'"
    );
    let (_, _, _, assessment) = analyze(files(&rust), &policy, true);
    assert!(assessment.diagnostics.is_empty());
    assert_eq!(
        assessment.report.requirements[0]
            .exception_reason
            .as_deref(),
        Some("Independent boundary cases")
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-GRANULARITY-002"))]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-GRANULARITY-003"))]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-QUALITY-001"))]
fn partial_discovery_proves_excess_without_certifying_absence() {
    let rust = (0..6)
        .map(|index| tagged(&format!("case{index}"), &[ID]))
        .collect::<String>();
    let (tree, context, _, assessment) = analyze(files(&rust), "", false);
    assert!(!assessment.report.discovery_complete);
    assert_eq!(assessment.diagnostics[0].code, "TooManyTestsForRequirement");
    let report = crate::report::analyze_tree(&tree, context.config);
    assert_eq!(report.version, 10);
    assert_eq!(
        report.completeness.relations,
        crate::report::StageState::Partial
    );
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|finding| finding.code == "MissingAutomatedTest")
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|finding| finding.code == "TooManyTestsForRequirement")
    );
    let (_, _, _, under) = analyze(files("#[test] fn untied() {}"), "", false);
    assert!(!under.report.discovery_complete);
    assert!(under.diagnostics.is_empty());
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-QUALITY-001"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-QUALITY-002"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-QUALITY-003"))]
fn untied_modes_and_named_glob_exceptions_preserve_the_inventory() {
    let rust = "#[test] fn utility() {} #[test] fn behavior() {}";
    let (_, _, _, advisory) = analyze(files(rust), "", true);
    assert_eq!(advisory.report.untied.len(), 2);
    assert!(advisory.diagnostics.is_empty());
    let exception = "[checks.specifications.untiedExceptions.'tests/**/*.rs']\nname = 'utility'\nreason = 'Test infrastructure'";
    let (_, _, _, denied) = analyze(
        files(rust),
        &format!("untiedTests = 'deny'\n{exception}"),
        true,
    );
    assert_eq!(denied.diagnostics[0].code, "UntiedTestsLimitExceeded");
    assert!(denied.diagnostics[0].message.starts_with("1 nonexempt"));
    assert_eq!(
        denied
            .report
            .untied
            .iter()
            .filter(|site| site.exception_reason.is_some())
            .count(),
        1
    );
    let (_, _, _, equal) = analyze(
        files(rust),
        &format!("untiedTests = 'deny'\nmaxUntiedTests = 1\n{exception}"),
        true,
    );
    assert!(equal.diagnostics.is_empty());
    let (_, _, output, off) = analyze(files(rust), "untiedTests = 'off'", true);
    assert!(off.report.untied.is_empty());
    assert_eq!(output.tests.len(), 2);
}

fn workspace_files(rust: &str) -> BTreeMap<String, String> {
    let mut files = files(rust);
    files.insert(
        "Cargo.toml".into(),
        "[workspace]\nmembers = ['crates/*']\nexclude = ['crates/fixture']\n".into(),
    );
    for name in ["app", "support", "fixture"] {
        files.insert(
            format!("crates/{name}/Cargo.toml"),
            format!("[package]\nname = '{name}'\nversion = '0.1.0'\n"),
        );
    }
    files
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-QUALITY-004"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-QUALITY-005"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-QUALITY-008"))]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-QUALITY-002"))]
fn ownership_uses_selected_manifests_and_external_integration_test_links() {
    let policy = "requireCrateOwnership = true\n[checks.specifications.crates.'crates/app/Cargo.toml']\nfeatures = ['DEMO/TESTS']\n[checks.specifications.crates.'crates/support/Cargo.toml']\nexclude = true\nreason = 'Test support only'";
    let (_, _, _, assessment) = analyze(
        workspace_files(&tagged("integration", &[ID, OTHER])),
        policy,
        true,
    );
    assert!(assessment.diagnostics.is_empty());
    assert!(assessment.report.ownership_complete);
    assert_eq!(assessment.report.crates.len(), 2);
    assert_eq!(assessment.report.crates[0].requirements, 2);
    assert_eq!(assessment.report.crates[0].linked_tests, 1);
    assert!(assessment.report.crates[1].excluded);
    let (_, _, _, unmapped) = analyze(workspace_files(""), "requireCrateOwnership = true", true);
    assert_eq!(
        unmapped
            .diagnostics
            .iter()
            .filter(|finding| finding.code == "MissingCrateOwnership")
            .count(),
        2
    );
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-QUALITY-006"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-QUALITY-007"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-QUALITY-009"))]
fn ownership_checks_minima_and_rejects_stale_targets() {
    let policy = "[checks.specifications.crates.'crates/app/Cargo.toml']\nfeatures = ['DEMO/TESTS']\nminRequirements = 3\nminLinkedTests = 1";
    let (_, _, _, assessment) = analyze(workspace_files(""), policy, true);
    assert_eq!(
        assessment
            .diagnostics
            .iter()
            .map(|finding| finding.code.as_str())
            .collect::<Vec<_>>(),
        ["InsufficientCrateRequirements", "InsufficientCrateTests"]
    );
    let (_, _, _, partial) = analyze(workspace_files(""), policy, false);
    assert!(
        !partial
            .diagnostics
            .iter()
            .any(|finding| finding.code == "InsufficientCrateTests")
    );
    for policy in [
        "[checks.specifications.crates.'crates/missing/Cargo.toml']\nfeatures = ['DEMO/TESTS']",
        "[checks.specifications.crates.'crates/app/Cargo.toml']\nfeatures = ['DEMO/UNKNOWN']",
        "[checks.specifications.requirementExceptions.'DEMO-TESTS-RULE-999']\nmaxTests = 9\nreason = 'Stale ID'",
    ] {
        let (_, _, _, assessment) = analyze(workspace_files(""), policy, true);
        assert!(
            assessment
                .diagnostics
                .iter()
                .any(|finding| finding.code == "SpecificationPolicyError")
        );
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-GRANULARITY-001"))]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-GRANULARITY-004"))]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-BREADTH-002"))]
fn caller_discovery_and_filesystem_reports_share_site_counts() {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "bloomery-quality-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let files = files(&tagged("bound", &[ID, OTHER]));
    for (path, source) in &files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, source).unwrap();
    }
    let (tree, context, output, assessment) = analyze(files, "", true);
    assert!(crate::validate_evidence(&context, &output).is_ok());
    assert_eq!(count(&assessment, ID), 1);
    let memory = crate::report::analyze_tree(&tree, context.config.clone());
    let filesystem = crate::report::analyze(&root, context.config);
    assert_eq!(memory, filesystem);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-QUALITY-001"))]
fn failed_source_discovery_never_publishes_a_quality_assessment() {
    let tree = MemoryTree::new(files("fn broken( {")).unwrap();
    let report = crate::report::analyze_tree(&tree, config(""));
    assert_eq!(
        report.completeness.discovery,
        crate::report::StageState::Failed
    );
    assert!(report.quality.is_none());
    assert!(!report.diagnostics.is_empty());
}
