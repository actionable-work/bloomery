use super::*;
use bloomery_workspace::source::MemoryTree;

const GROUP: &str = ".bloomery/specs/DEMO/TESTS/requirements/BIND.toml";

fn report(source: &str) -> StaticReport {
    let mut files = tests::files();
    files.insert(GROUP.into(), source.into());
    analyze_tree(&MemoryTree::new(files).unwrap(), tests::config())
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-016"))]
fn versioned_groups_preserve_meaning_and_bind_authored_bytes() {
    let original = tests::files().remove(GROUP).unwrap();
    let implicit = report(&original);
    let source = format!("schema_version = 1\n{original}");
    let explicit = report(&source);
    assert_eq!(implicit.requirements[0].id, explicit.requirements[0].id);
    assert_eq!(implicit.requirements[0].coverage, Coverage::Bound);
    assert_eq!(implicit.tests, explicit.tests);
    assert_eq!(implicit.bindings, explicit.bindings);
    assert_eq!(implicit.scope, explicit.scope);
    assert_eq!(implicit.completeness, explicit.completeness);
    assert_eq!(implicit.inputs.configuration, explicit.inputs.configuration);
    assert_eq!(implicit.inputs.scope, explicit.inputs.scope);
    assert_ne!(implicit.inputs.tree, explicit.inputs.tree);
    assert_ne!(
        implicit.inputs.specifications,
        explicit.inputs.specifications
    );
    assert_ne!(implicit.identity, explicit.identity);
    assert_eq!(explicit, report(&source));
    let mut files = tests::files();
    files.insert(GROUP.into(), source);
    let reversed = MemoryTree::new(files.into_iter().rev()).unwrap();
    assert_eq!(explicit, analyze_tree(&reversed, tests::config()));
    assert_eq!(explicit.version, 7);
    assert_eq!(
        explicit.inputs.specification_schema,
        "bloomery-grouped-specifications-v1"
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-016"))]
fn unsupported_and_malformed_versions_fail_specs_without_hiding_discovery() {
    let original = tests::files().remove(GROUP).unwrap();
    let valid = report(&original);
    for (source, code) in [
        (
            format!("schema_version = 999\n{original}"),
            "UnsupportedSpecificationSchema",
        ),
        (
            format!("schema_version = -1\n{original}"),
            "UnsupportedSpecificationSchema",
        ),
        (format!("schema_version = \"1\"\n{original}"), "ParseError"),
        (format!("schema_version = true\n{original}"), "ParseError"),
        (
            format!("schema_version = 1\nschema_version = 1\n{original}"),
            "ParseError",
        ),
        (
            original.replace("[[requirements]]", "[[requirements]]\nschema_version = 1"),
            "ParseError",
        ),
        (
            original.replace("ears = {", "ears = { schema_version = 1,"),
            "ParseError",
        ),
    ] {
        let failed = report(&source);
        assert_eq!(failed.completeness.specifications, StageState::Failed);
        assert_eq!(failed.completeness.discovery, StageState::Complete);
        assert_eq!(failed.completeness.relations, StageState::NotAttempted);
        assert!(failed.inputs.specifications.is_none());
        assert!(failed.requirements.is_empty());
        assert_eq!(failed.tests, valid.tests);
        assert!(
            failed
                .bindings
                .iter()
                .all(|binding| binding.resolution == BindingResolution::SpecificationsUnavailable)
        );
        let finding = failed
            .diagnostics
            .iter()
            .find(|finding| finding.code == code)
            .unwrap();
        assert_eq!(finding.location.as_ref().unwrap().path, GROUP);
        assert!(finding.location.as_ref().unwrap().line.is_some());
        assert_eq!(failed, report(&source));
    }
    let mut files = tests::files();
    files.insert(GROUP.into(), format!("schema_version = 999\n{original}"));
    let mut config = tests::config();
    config.specs.enable = Some(false);
    let disabled = analyze_tree(&MemoryTree::new(files).unwrap(), config);
    assert_eq!(disabled.completeness.specifications, StageState::Disabled);
    assert!(disabled.diagnostics.is_empty());
    assert_eq!(disabled.tests, valid.tests);
}
