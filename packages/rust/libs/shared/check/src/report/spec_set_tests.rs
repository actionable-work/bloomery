use super::*;
use bloomery_workspace::source::MemoryTree;
use std::collections::BTreeMap;

const GROUP: &str = ".bloomery/specs/DEMO/TESTS/requirements/BIND.toml";
const ID: &str = "DEMO-TESTS-BIND-001";

fn analyze(files: BTreeMap<String, String>) -> StaticReport {
    analyze_tree(&MemoryTree::new(files).unwrap(), tests::config())
}

fn group(source: String) -> StaticReport {
    let mut files = tests::files();
    files.insert(GROUP.into(), source);
    analyze(files)
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-017"))]
fn revisions_ignore_formatting_positions_and_equivalent_design_spelling() {
    let original = analyze(tests::files());
    // Fixed canonical JSON vector, independently calculated from the fixture.
    assert_eq!(
        original.requirements[0].revision,
        "sha256:3ed0815f4c649dbaa661cccb4d6bc72292b12b66da917ea3716f4333708182f6"
    );
    let source = tests::files().remove(GROUP).unwrap();
    let formatted = group(format!(
        "# café 🦀\r\nschema_version = 1\r\n{}",
        source.replace("title = \"Binding\"", "title='Binding' # same meaning")
    ));
    assert_eq!(
        original.requirements[0].revision,
        formatted.requirements[0].revision
    );
    assert_ne!(
        original.requirements[0].location,
        formatted.requirements[0].location
    );
    assert_ne!(original.spec_set, formatted.spec_set);
    let implicit = group(source.replace("design = \"design/main.md\"\n", ""));
    let explicit = group(source.replace("design/main.md", "README.md"));
    assert_eq!(
        implicit.requirements[0].revision,
        explicit.requirements[0].revision
    );
    let dot = group(source.replace("design/main.md", "./design/main.md"));
    assert_eq!(
        original.requirements[0].revision,
        dot.requirements[0].revision
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-017"))]
fn every_record_field_and_typed_ears_form_affects_revision() {
    let original = analyze(tests::files());
    let source = tests::files().remove(GROUP).unwrap();
    let mut revisions = std::collections::BTreeSet::new();
    revisions.insert(original.requirements[0].revision.clone());
    for (from, to) in [
        (ID, "DEMO-TESTS-BIND-002"),
        ("title = \"Binding\"", "title = \"Changed\""),
        ("manual = false", "manual = true"),
        ("design/main.md", "README.md"),
        ("design/main.md", "design/main.md#main"),
        ("system = \"example\"", "system = \"changed\""),
        ("action = \"work\"", "action = \"change\""),
        (
            "type = \"ubiquitous\"",
            "type = \"event\", trigger = \"ready\"",
        ),
        (
            "type = \"ubiquitous\"",
            "type = \"state\", state = \"ready\"",
        ),
        (
            "type = \"ubiquitous\"",
            "type = \"unwanted_behavior\", trigger = \"ready\"",
        ),
        (
            "type = \"ubiquitous\"",
            "type = \"optional\", feature = \"ready\"",
        ),
        (
            "type = \"ubiquitous\"",
            "type = \"complex\", state = \"ready\", trigger = \"called\"",
        ),
    ] {
        let changed = group(source.replace(from, to));
        assert_eq!(
            changed.completeness.specifications,
            StageState::Complete,
            "{to}"
        );
        assert!(
            revisions.insert(changed.requirements[0].revision.clone()),
            "{to}"
        );
        assert_ne!(
            original.spec_set.as_ref().unwrap().identity,
            changed.spec_set.unwrap().identity
        );
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-018"))]
fn manifest_includes_manual_and_unbound_records_and_has_canonical_identity() {
    let source = tests::files().remove(GROUP).unwrap();
    let second = tests::record("DEMO-TESTS-BIND-002", false);
    let third = tests::record("DEMO-TESTS-BIND-003", true);
    let report = group(format!("{source}{second}{third}"));
    let manifest = report.spec_set.as_ref().unwrap();
    assert_eq!(manifest.version, 1);
    assert_eq!(manifest.source_root, ".bloomery/specs");
    assert_eq!(
        manifest.source_identity,
        *report.inputs.specifications.as_ref().unwrap()
    );
    assert_eq!(manifest.configuration, report.inputs.configuration);
    assert_eq!(manifest.scope, report.inputs.scope);
    assert_eq!(manifest.completeness, report.completeness);
    assert_eq!(manifest.active.len(), 3);
    for requirement in &report.requirements {
        assert_eq!(manifest.active[&requirement.id], requirement.revision);
    }
    assert_eq!(report.requirements[1].coverage, Coverage::Missing);
    assert_eq!(report.requirements[2].coverage, Coverage::Manual);
    let reordered = group(format!(
        "group = \"BIND\"\n{third}{second}{}",
        tests::record(ID, false)
    ));
    assert_eq!(manifest.active, reordered.spec_set.unwrap().active);
    let mut payload = serde_json::to_value(manifest).unwrap();
    payload.as_object_mut().unwrap().remove("identity");
    assert_eq!(manifest.identity, identity(&payload));
    let reversed = MemoryTree::new(tests::files().into_iter().rev()).unwrap();
    assert_eq!(
        analyze(tests::files()),
        analyze_tree(&reversed, tests::config())
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-018"))]
fn manifest_changes_with_validation_inputs_without_changing_record_revisions() {
    let original = analyze(tests::files());
    let expected = original.spec_set.as_ref().unwrap();
    let mut unrelated = tests::files();
    unrelated.insert("asset.bin".into(), "new tree bytes".into());
    let unrelated = analyze(unrelated);
    assert_ne!(original.inputs.tree, unrelated.inputs.tree);
    assert_eq!(original.spec_set, unrelated.spec_set);
    let mut design = tests::files();
    design.insert(
        ".bloomery/specs/DEMO/TESTS/design/main.md".into(),
        "# Main\nRevised design\n".into(),
    );
    let mut bindings = tests::files();
    bindings.insert("tests/a.rs".into(), "#[test] fn untied() {}".into());
    let mut findings = tests::files();
    findings.get_mut("tests/a.rs").unwrap().push_str(
        "#[cfg_attr(any(), bloomery(\"DEMO-TESTS-BIND-999\"))]\n#[test] fn orphan() {}\n",
    );
    let tree = MemoryTree::new(tests::files()).unwrap();
    let mut disabled = tests::config();
    disabled.scanners.rust.enabled = false;
    let relocated = MemoryTree::new(
        tests::files()
            .into_iter()
            .map(|(path, text)| (path.replace(".bloomery/specs/", ".bloomery/custom/"), text)),
    )
    .unwrap();
    let mut custom = tests::config();
    custom.specs.dir = "custom".into();
    for report in [
        analyze(design),
        analyze(bindings),
        analyze(findings),
        analyze_tree(&tree, disabled),
        analyze_tree_with_cfg(&tree, tests::config(), RustCfg::default()),
        analyze_tree(&relocated, custom),
    ] {
        let manifest = report.spec_set.unwrap();
        assert_eq!(expected.active, manifest.active);
        assert_ne!(expected.identity, manifest.identity);
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-019"))]
fn manifest_preserves_disabled_and_incomplete_analysis_without_partial_spec_sets() {
    let source = tests::files().remove(GROUP).unwrap();
    for invalid in [
        "invalid [".into(),
        format!("{source}{}", tests::record(ID, false)),
    ] {
        let failed = group(invalid);
        assert_eq!(failed.completeness.specifications, StageState::Failed);
        assert!(failed.spec_set.is_none());
    }
    let tree = MemoryTree::new(tests::files()).unwrap();
    let mut config = tests::config();
    config.specs.enable = Some(false);
    let disabled = analyze_tree(&tree, config);
    let manifest = disabled.spec_set.unwrap();
    assert!(manifest.active.is_empty());
    assert_eq!(manifest.completeness.specifications, StageState::Disabled);
    let mut files = tests::files();
    files.insert("tests/a.rs".into(), "fn broken( {".into());
    let failed = analyze(files);
    let manifest = failed.spec_set.unwrap();
    assert_eq!(manifest.active.len(), 1);
    assert_eq!(manifest.completeness.discovery, StageState::Failed);
    assert_eq!(manifest.completeness.relations, StageState::NotAttempted);
    let mut config = tests::config();
    config.scanners.rust.enabled = false;
    let partial = analyze_tree(&tree, config);
    let manifest = partial.spec_set.unwrap();
    assert_eq!(manifest.active.len(), 1);
    assert_eq!(manifest.completeness.discovery, StageState::Partial);
    assert_eq!(manifest.completeness.relations, StageState::Partial);
}
