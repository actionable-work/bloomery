use super::*;
use crate::{Config, Diagnostic, catalog, config::parse_contents};
use std::path::Path;

fn parse(policy: &str) -> Result<Config, Diagnostic> {
    parse_contents(
        &format!("version = 3\n[checks.specifications]\n{policy}"),
        Path::new(".bloomery/config.toml"),
    )
    .map(|loaded| loaded.config)
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-GRANULARITY-001"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-GRANULARITY-002"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-QUALITY-001"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-BREADTH-001"))]
fn defaults_and_catalog_agree_on_the_effective_policy() {
    let policy = from_config(&parse("").unwrap()).unwrap();
    assert_eq!(policy.max_tests_per_requirement, 5);
    assert_eq!(policy.max_requirements_per_test, 5);
    assert_eq!(policy.untied_tests, UntiedMode::Report);
    assert_eq!(policy.max_untied_tests, 0);
    assert!(!policy.require_crate_ownership);
    assert_eq!(
        catalog::lookup(&["checks", "specifications", "maxTestsPerRequirement"])
            .unwrap()
            .recommended,
        Some(catalog::DefaultValue::Integer(5))
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-GRANULARITY-003"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-GRANULARITY-004"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-QUALITY-002"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-BREADTH-002"))]
fn invalid_limits_and_exception_shapes_are_rejected_when_disabled() {
    for value in ["0", "-1", "1.5", "true", "'5'"] {
        assert!(parse(&format!("enable = false\nmaxRequirementsPerTest = {value}")).is_err());
        let error =
            parse(&format!("enable = false\nmaxTestsPerRequirement = {value}")).unwrap_err();
        assert_eq!(error.code, "ConfigurationError");
    }
    for policy in [
        "untiedTests = 'warn'",
        "maxUntiedTests = -1",
        "[checks.specifications.testExceptions.bad]\npath = 'tests/a.rs'\nmaxRequirements = 6",
        "[checks.specifications.testExceptions.bad]\npath = '../a.rs'\nmaxRequirements = 6\nreason = 'Why'",
        "[checks.specifications.testExceptions.bad]\npath = 'tests/a.rs'\nmaxRequirements = 0\nreason = 'Why'",
        "[checks.specifications.testExceptions.bad]\npath = 'tests/a.rs'\nmaxRequirements = 6\nreason = 'Why'\nname = ''",
        "[checks.specifications.testExceptions.' ']\npath = 'tests/a.rs'\nmaxRequirements = 6\nreason = 'Why'",
        "[checks.specifications.testExceptions.bad]\npath = 'tests/a.rs'\nmaxRequirements = 6\nreason = 'Why'\nunknown = true",
        "[checks.specifications.requirementExceptions.'DEMO-TESTS-RULE-001']\nmaxTests = 9",
        "[checks.specifications.requirementExceptions.'bad']\nmaxTests = 9\nreason = 'Why'",
        "[checks.specifications.requirementExceptions.'DEMO-TESTS-RULE-01']\nmaxTests = 9\nreason = 'Invalid sequence width'",
        "[checks.specifications.untiedExceptions.'../tests/*']\nreason = 'Why'",
        "[checks.specifications.untiedExceptions.'tests/*']\nreason = '  '",
        "[checks.specifications.untiedExceptions.'tests/*']\nreason = 'Why'\nname = ''",
        "[checks.specifications.crates.'crates/a/Cargo.toml']\nfeatures = ['DEMO/TESTS', 'DEMO/TESTS']",
        "[checks.specifications.crates.'crates/a/Cargo.toml']\nfeatures = ['DEMO']",
        "[checks.specifications.crates.'crates/a/Cargo.toml']\nexclude = true",
        "[checks.specifications.crates.'crates/a/Cargo.toml']\nexclude = true\nfeatures = ['DEMO/TESTS']\nreason = 'Why'",
        "[checks.specifications.crates.'crates/a/Cargo.toml']\nfeatures = ['DEMO/TESTS']\nunknown = true",
    ] {
        assert!(
            parse(&format!("enable = false\n{policy}")).is_err(),
            "{policy}"
        );
    }
    let mut explicit = Config::default();
    explicit.checks = Some(toml::from_str("[specifications]\nmaxTestsPerRequirement = 0").unwrap());
    assert!(explicit.validate().is_err());
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-QUALITY-003"))]
fn parameterized_schema_keys_are_editable_without_recommendations() {
    for path in [
        vec![
            "checks",
            "specifications",
            "requirementExceptions",
            "DEMO-TESTS-RULE-001",
            "maxTests",
        ],
        vec![
            "checks",
            "specifications",
            "untiedExceptions",
            "tests/**/*.rs",
            "reason",
        ],
        vec![
            "checks",
            "specifications",
            "crates",
            "crates/a/Cargo.toml",
            "features",
        ],
    ] {
        let entry = catalog::resolve(&path).expect("parameterized key");
        assert!(entry.recommended.is_none());
        assert!(!entry.documentation.is_empty());
    }
    let policy = "[checks.specifications.requirementExceptions.'DEMO-TESTS-RULE-001']\nmaxTests = 9\nreason = 'Many boundaries'\n[checks.specifications.untiedExceptions.'tests/**/*.rs']\nreason = 'Infrastructure'\n[checks.specifications.crates.'crates/a/Cargo.toml']\nfeatures = ['DEMO/TESTS']\nminLinkedTests = 0";
    assert!(parse(policy).is_ok());
    assert!(parse("[checks.specifications.requirementExceptions.'_DEMO-2D-RULE-0001']\nmaxTests = 9\nreason = 'Valid identifier grammar'\n[checks.specifications.crates.'crates/a/Cargo.toml']\nfeatures = ['_DEMO/2D']").is_ok());
}
