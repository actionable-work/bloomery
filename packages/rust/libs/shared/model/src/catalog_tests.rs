use super::{
    BUILTIN_FORMATTERS, CATALOG, CATALOG_VERSION, DefaultValue, ValueType, effective_default,
    effective_value, lookup, recommended_entries, resolve, validate_raw,
};
use toml::Value;

fn path(entry: &super::CatalogEntry) -> String {
    entry.path.join(".")
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-001"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-023"))]
fn catalog_enumerates_every_recognized_key() {
    let mut paths = CATALOG.iter().map(path).collect::<Vec<_>>();
    let count = paths.len();
    paths.sort();
    paths.dedup();
    assert_eq!(paths.len(), count, "catalog paths must be unique");
    for expected in [
        "specs.enable",
        "specs.dir",
        "scanners.rust.enabled",
        "build.cargoToml",
        "build.toolchain.rustc",
        "build.profile.release.optLevel",
        "build.profile.dev.debuginfo",
        "build.profile.test.debuginfo",
        "build.flags.rustc",
        "devShell.shellHook",
        "checks.enable",
        "checks.profile",
        "build.unify",
        "formatters.toml-sort.before",
        "checks.codeHealth.enable",
        "checks.specifications.enable",
        "checks.clippySuppressions.enable",
        "checks.clippySuppressions.allow",
        "checks.unusedDependencies.enable",
        "checks.unusedDependencies.exceptions",
        "checks.codeHealth.duplicationFreeRatio",
        "checks.codeHealth.maxNesting",
    ] {
        assert!(
            paths.iter().any(|candidate| candidate == expected),
            "{expected}"
        );
    }
    assert!(
        resolve(&["version"]).is_none(),
        "format metadata is not catalogued"
    );
    let versioned: Value =
        toml::from_str("version = 3\n[checks]\nenable = true\n").expect("versioned TOML");
    assert!(
        validate_raw(&versioned).is_ok(),
        "format metadata is ignored by the catalog"
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-HEALTH-001"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-HEALTH-005"))]
fn health_schema_supplies_all_recommended_defaults() {
    let health_entries = CATALOG
        .iter()
        .filter(|entry| entry.path.starts_with(&["checks", "codeHealth"]))
        .collect::<Vec<_>>();
    assert_eq!(health_entries.len(), 20);
    assert!(
        health_entries
            .iter()
            .all(|entry| entry.recommended.is_some())
    );
    assert_eq!(
        effective_default(&["checks", "codeHealth", "duplicationFreeRatio"]),
        Some(Value::Float(0.02))
    );
    assert_eq!(
        effective_default(&["checks", "codeHealth", "maxRegression"]),
        Some(Value::Integer(2))
    );
    assert_eq!(
        effective_default(&["checks", "codeHealth", "crateSlocLimit"]),
        Some(Value::Integer(1_500))
    );
    assert_eq!(
        effective_default(&["checks", "codeHealth", "maxCrateSloc"]),
        Some(Value::Integer(5_000))
    );
    for (key, expected) in [
        ("complexity", 25),
        ("size", 20),
        ("nesting", 10),
        ("duplication", 15),
        ("crateSize", 30),
    ] {
        assert_eq!(
            effective_default(&["checks", "codeHealth", "weights", key]),
            Some(Value::Integer(expected))
        );
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-HEALTH-002"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-HEALTH-003"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-HEALTH-006"))]
fn health_schema_rejects_invalid_policy_even_when_disabled() {
    for raw in [
        "[checks.codeHealth]\nenable=false\nminCrateScore=101\n",
        "[checks.codeHealth]\nduplicationFreeRatio=1.0\n",
        "[checks.codeHealth]\nduplicateMinTokens=15\n",
        "[checks.codeHealth]\ncyclomaticLimit=10\nmaxCyclomatic=9\n",
        "[checks.codeHealth]\nunknown=true\n",
        "[checks.codeHealth]\nminCrateScore=nan\n",
        "[checks.codeHealth]\ncrateSlocLimit=1501\nmaxCrateSloc=1500\n",
        "[checks.codeHealth.weights]\ncomplexity=100.001\n",
        "[checks.codeHealth.weights]\ncomplexity=101\n",
        "[checks.codeHealth]\nenable=false\n[checks.codeHealth.weights]\ncomplexity=0\nsize=0\nnesting=0\nduplication=0\ncrateSize=0\n",
        "[checks.codeHealth.weights]\ncomplexity=nan\n",
        "[checks.codeHealth.weights]\nunknown=25\n",
    ] {
        let value = toml::from_str::<Value>(raw).expect("TOML parses");
        assert!(validate_raw(&value).is_err(), "accepted {raw}");
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-HEALTH-007"))]
fn health_crate_exception_keys_are_catalogued_and_validated() {
    let key = resolve(&[
        "checks",
        "codeHealth",
        "crateExceptions",
        "packages/demo/Cargo.toml",
        "maxCrateSloc",
    ])
    .expect("crate exception hard limit");
    assert_eq!(key.value_type, ValueType::PositiveInteger);
    assert!(key.recommended.is_none());

    for raw in [
        "[checks.codeHealth.crateExceptions.\"../demo/Cargo.toml\"]\nmaxCrateSloc=6000\n",
        "[checks.codeHealth.crateExceptions.\"packages/demo/Cargo.toml\"]\nmaxCrateSloc=1499\n",
        "[checks.codeHealth.crateExceptions.\"packages/demo/Cargo.toml\"]\nunknown=6000\n",
    ] {
        let value = toml::from_str::<Value>(raw).expect("TOML parses");
        assert!(validate_raw(&value).is_err(), "accepted {raw}");
    }

    let valid = toml::from_str::<Value>(
        "[checks.codeHealth.crateExceptions.\"packages/demo/Cargo.toml\"]\nmaxCrateSloc=6000\n",
    )
    .expect("valid TOML");
    assert!(validate_raw(&valid).is_ok());
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-UNUSED-001"))]
fn unused_dependency_policy_is_catalogued() {
    for (path, value_type, recommended) in [
        (
            &["checks", "unusedDependencies", "enable"][..],
            ValueType::Bool,
            DefaultValue::Bool(true),
        ),
        (
            &["checks", "unusedDependencies", "exceptions"][..],
            ValueType::StringList,
            DefaultValue::StringList(&[]),
        ),
    ] {
        let entry = lookup(path).expect("unused dependency key is catalogued");
        assert_eq!(entry.value_type, value_type);
        assert_eq!(entry.recommended, Some(recommended));
        assert!(!entry.documentation.is_empty());
    }
    assert_eq!(
        effective_default(&["checks", "unusedDependencies", "enable"]),
        Some(Value::Boolean(false))
    );
    assert_eq!(
        effective_default(&["checks", "unusedDependencies", "exceptions"]),
        Some(Value::Array(Vec::new()))
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-UNUSED-006"))]
fn compiler_unused_dependency_switch_is_catalogued_and_validated() {
    let path = &["checks", "unusedDependencies", "compiler"][..];
    let entry = lookup(path).expect("compiler switch is catalogued");
    assert_eq!(entry.value_type, ValueType::Bool);
    assert_eq!(entry.recommended, Some(DefaultValue::Bool(true)));
    assert_eq!(effective_default(path), Some(Value::Boolean(false)));
    let invalid = toml::from_str::<Value>("[checks.unusedDependencies]\ncompiler = \"true\"\n")
        .expect("TOML parses");
    assert!(validate_raw(&invalid).is_err());
    let valid =
        toml::from_str::<Value>("[checks.unusedDependencies]\nenable = false\ncompiler = true\n")
            .expect("TOML parses");
    assert!(validate_raw(&valid).is_ok());
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-UNUSED-003"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-UNUSED-004"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-UNUSED-005"))]
fn unused_dependency_policy_is_validated_even_when_disabled() {
    for raw in [
        "[checks.unusedDependencies]\nenable = false\nunknown = true\n",
        "[checks.unusedDependencies]\nenable = false\nexceptions = [\"serde@1.0.0\"]\n",
        "[checks.unusedDependencies]\nenable = false\nexceptions = [\"-serde\"]\n",
        "[checks.unusedDependencies]\nenable = false\nexceptions = [\"serde\", \"serde\"]\n",
        "[checks.unusedDependencies]\nenable = \"false\"\n",
        "[checks.unusedDependencies]\nexceptions = \"serde\"\n",
    ] {
        let value = toml::from_str::<Value>(raw).expect("TOML parses");
        assert!(validate_raw(&value).is_err(), "accepted {raw}");
    }
    let valid = toml::from_str::<Value>(
        "[checks.unusedDependencies]\nenable = false\nexceptions = [\"openssl-sys\", \"serde_json\"]\n",
    )
    .expect("TOML parses");
    assert!(validate_raw(&valid).is_ok());
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-SUPPRESSIONS-001"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-SUPPRESSIONS-003"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-SUPPRESSIONS-004"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-SUPPRESSIONS-005"))]
fn clippy_suppression_policy_is_catalogued_and_validated_even_when_disabled() {
    assert_eq!(
        effective_default(&["checks", "clippySuppressions", "enable"]),
        Some(Value::Boolean(false))
    );
    assert_eq!(
        effective_default(&["checks", "clippySuppressions", "allow"]),
        Some(Value::Array(Vec::new()))
    );
    for raw in [
        "[checks.clippySuppressions]\nenable = false\nunknown = true\n",
        "[checks.clippySuppressions]\nenable = false\nallow = [\"unwrap_used\"]\n",
        "[checks.clippySuppressions]\nenable = false\nallow = [\"clippy::unwrap_used\", \"clippy::unwrap_used\"]\n",
        "[checks.clippySuppressions]\nenable = \"false\"\n",
    ] {
        let value = toml::from_str::<Value>(raw).expect("TOML parses");
        assert!(validate_raw(&value).is_err(), "accepted {raw}");
    }
    let valid = toml::from_str::<Value>(
        "[checks.clippySuppressions]\nenable = false\nallow = [\"clippy::unwrap_used\"]\n",
    )
    .expect("TOML parses");
    assert!(validate_raw(&valid).is_ok());
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-002"))]
fn catalog_entries_pair_paths_with_types() {
    assert_eq!(
        lookup(&["specs", "enable"]).unwrap().value_type,
        ValueType::Bool
    );
    assert_eq!(
        lookup(&["checks", "enable"]).unwrap().value_type,
        ValueType::Bool
    );
    assert_eq!(
        lookup(&["checks", "workspaceDependencies", "enable"])
            .unwrap()
            .value_type,
        ValueType::Bool
    );
    assert_eq!(
        lookup(&["checks", "noDefaultFeatures", "enable"])
            .unwrap()
            .value_type,
        ValueType::Bool
    );
    assert_eq!(
        lookup(&["scanners", "rust", "paths"]).unwrap().value_type,
        ValueType::StringList
    );
    assert_eq!(
        lookup(&["build", "toolchain", "rustc"]).unwrap().value_type,
        ValueType::Package
    );
    assert_eq!(
        lookup(&["build", "toolchain", "linker"])
            .unwrap()
            .value_type,
        ValueType::Union(&[ValueType::StringEnum(&["lld", "mold", "system"])])
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-003"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-019"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-024"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-025"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-026"))]
fn effective_defaults_are_distinct_from_upgrade_recommendations() {
    assert_eq!(
        effective_default(&["specs", "dir"]),
        Some(Value::String("specs".into()))
    );
    assert_eq!(
        effective_default(&["scanners", "rust", "enabled"]),
        Some(Value::Boolean(true))
    );
    assert_eq!(
        effective_default(&["scanners", "rust", "paths"]),
        Some(Value::Array(vec![Value::String(
            "packages/rust/**/*.rs".into()
        )]))
    );
    assert_eq!(
        effective_default(&["build", "flags", "rustc"]),
        Some(Value::Array(Vec::new()))
    );
    assert_eq!(
        effective_default(&["checks", "profile"]),
        Some(Value::String("dev".into()))
    );
    assert_eq!(
        effective_default(&["checks", "packages", "enable"]),
        Some(Value::Boolean(false))
    );
    assert_eq!(
        effective_default(&["checks", "workspaceDependencies", "enable"]),
        Some(Value::Boolean(false))
    );
    assert_eq!(
        effective_default(&["checks", "noDefaultFeatures", "enable"]),
        Some(Value::Boolean(false))
    );
    assert_eq!(
        effective_default(&["checks", "vulnerabilities", "enable"]),
        Some(Value::Boolean(false))
    );
    assert_eq!(
        lookup(&["checks", "vulnerabilities", "enable"])
            .unwrap()
            .recommended,
        None
    );
    for path in [
        &["checks", "enable"][..],
        &["checks", "packages", "enable"][..],
        &["checks", "crates", "enable"][..],
        &["checks", "lockFreshness", "enable"][..],
        &["checks", "workspaceDependencies", "enable"][..],
        &["checks", "noDefaultFeatures", "enable"][..],
        &["checks", "bans", "enable"][..],
        &["checks", "licenses", "enable"][..],
        &["checks", "codeHealth", "enable"][..],
        &["checks", "specifications", "enable"][..],
        &["checks", "clippySuppressions", "enable"][..],
        &["checks", "unusedDependencies", "enable"][..],
    ] {
        let entry = lookup(path).expect("check control is catalogued");
        assert_eq!(entry.recommended, Some(DefaultValue::Bool(true)));
        assert_eq!(effective_default(path), Some(Value::Boolean(false)));
    }
    assert_eq!(
        effective_default(&["checks", "clippySuppressions", "allow"]),
        Some(Value::Array(Vec::new()))
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-027"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-028"))]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-029"))]
fn dependency_policy_keys_and_license_overrides_are_catalogued() {
    for path in [
        &["checks", "vulnerabilities", "ignoreAdvisories"][..],
        &["checks", "vulnerabilities", "informational", "unmaintained"][..],
        &["checks", "bans", "allow"][..],
        &["checks", "bans", "deny"][..],
        &["checks", "bans", "exceptions"][..],
        &["checks", "licenses", "allow"][..],
        &["checks", "licenses", "deny"][..],
        &["checks", "licenses", "exceptions"][..],
    ] {
        assert!(lookup(path).is_some(), "{}", path.join("."));
    }
    assert_eq!(
        lookup(&["checks", "vulnerabilities", "informational", "unmaintained"])
            .unwrap()
            .value_type,
        ValueType::StringEnum(&["warn", "deny", "ignore"])
    );

    let override_key = resolve(&["checks", "licenses", "overrides", "legacy-crate@1.2.3"])
        .expect("parameterized license override");
    assert_eq!(override_key.value_type, ValueType::String);
    assert_eq!(override_key.recommended, None);
    assert!(resolve(&["checks", "licenses", "overrides", "legacy-crate@latest"]).is_none());

    let valid = toml::from_str::<Value>(
            "[checks.bans]\nallow = [\"serde\", \"foo-bar@1.2.3\"]\n[checks.vulnerabilities]\nignoreAdvisories = [\"RUSTSEC-2024-0001\"]\n[checks.licenses.overrides]\nlegacy = \"MIT OR Apache-2.0\"\n",
        )
        .expect("valid policy TOML");
    assert!(validate_raw(&valid).is_ok());
    for raw in [
        "[checks.bans]\nallow = [\"bad selector@latest\"]\n",
        "[checks.vulnerabilities]\nignoreAdvisories = [\"CVE-2024-0001\"]\n",
        "[checks.vulnerabilities.informational]\nnotice = \"fail\"\n",
    ] {
        let value = toml::from_str::<Value>(raw).expect("TOML parses");
        assert!(validate_raw(&value).is_err(), "accepted {raw}");
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-022"))]
fn crate_check_recommends_on_while_defaulting_off() {
    assert_eq!(
        lookup(&["checks", "crates", "enable"]).unwrap().recommended,
        Some(DefaultValue::Bool(true))
    );
    assert_eq!(
        effective_default(&["checks", "crates", "enable"]),
        Some(Value::Boolean(false))
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-004"))]
fn catalog_covers_the_build_tables() {
    for prefix in ["build", "devShell", "checks", "features", "formatters"] {
        assert!(
            CATALOG.iter().any(|entry| entry.path[0] == prefix),
            "missing {prefix}"
        );
    }
    for path in [
        &["build", "toolchain", "rustc"][..],
        &["build", "profile", "release", "optLevel"][..],
        &["build", "flags", "rustc"][..],
    ] {
        assert!(lookup(path).is_some(), "missing {}", path.join("."));
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-014"))]
fn scanners_are_enabled_by_default() {
    for path in ["rust", "playwright", "nix"] {
        assert_eq!(
            effective_default(&["scanners", path, "enabled"]),
            Some(Value::Boolean(true)),
            "scanners.{path}.enabled"
        );
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-015"))]
fn scanner_paths_have_repository_relative_defaults() {
    for (scanner, expected) in [
        ("rust", vec!["packages/rust/**/*.rs"]),
        (
            "playwright",
            vec![
                "packages/playwright/**/*.spec.ts",
                "packages/playwright/**/*.test.ts",
            ],
        ),
        (
            "nix",
            vec!["*.nix", "lib/**/*.nix", "nix/**/*.nix", "tests/**/*.nix"],
        ),
    ] {
        let expected = expected
            .into_iter()
            .map(|value| Value::String(value.to_owned()))
            .collect::<Vec<_>>();
        assert_eq!(
            effective_default(&["scanners", scanner, "paths"]),
            Some(Value::Array(expected)),
            "scanners.{scanner}.paths"
        );
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-016"))]
fn nix_test_paths_are_catalogued() {
    let entry = resolve(&["scanners", "nix", "testPaths"]).expect("testPaths entry");
    assert_eq!(entry.value_type, ValueType::StringList);
    assert_eq!(
        entry.recommended,
        Some(DefaultValue::StringList(&["**/*.test.nix"]))
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-005"))]
fn catalog_version_tracks_the_binary() {
    assert_eq!(CATALOG_VERSION, 18);
    assert!(!CATALOG.is_empty());
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-020"))]
fn spec_management_enablement_is_catalogued() {
    let entry = lookup(&["specs", "enable"]).expect("specs.enable is catalogued");
    assert_eq!(entry.value_type, ValueType::Bool);
    assert!(!entry.documentation.is_empty());
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-021"))]
fn spec_management_enablement_has_no_recommendation() {
    assert_eq!(lookup(&["specs", "enable"]).unwrap().recommended, None);
    assert_eq!(effective_default(&["specs", "enable"]), None);
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-006"))]
fn optional_keys_have_no_recommendation() {
    for path in [
        "build.bloomeryLock",
        "build.members",
        "build.toolchain.linker",
        "features.cratesIoIndex",
        "build.profile.release.optLevel",
        "build.profile.dev.linkArgs",
        "build.profile.test.debuginfo",
        "specs.enable",
    ] {
        let parts = path.split('.').collect::<Vec<_>>();
        assert_eq!(lookup(&parts).unwrap().recommended, None, "{path}");
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-007"))]
fn materialization_uses_recommended_keys_only() {
    let recommended = recommended_entries().collect::<Vec<_>>();
    assert!(!recommended.is_empty());
    assert!(recommended.iter().all(|entry| entry.recommended.is_some()));
    assert!(
        !recommended
            .iter()
            .any(|entry| path(entry) == "build.profile.release.optLevel")
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-008"))]
fn explicit_values_take_precedence_over_recommendations() {
    let raw: Value = toml::from_str("[checks]\nenable = false\n").expect("toml");
    assert_eq!(
        effective_value(Some(&raw), &["checks", "enable"]),
        Some(Value::Boolean(false))
    );
    assert_eq!(
        effective_value(Some(&raw), &["checks", "lockFreshness", "enable"]),
        Some(Value::Boolean(false))
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-009"))]
fn catalog_entries_carry_documentation() {
    for entry in CATALOG {
        assert!(
            !entry.documentation.trim().is_empty(),
            "{} has no documentation",
            path(entry)
        );
    }
    assert_eq!(BUILTIN_FORMATTERS.len(), 8);
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-010"))]
fn catalog_covers_the_parameterized_formatter_table() {
    let known = resolve(&["formatters", "toml-sort", "before"]).expect("built-in formatter");
    assert_eq!(known.value_type, ValueType::StringList);
    assert!(known.recommended.is_some());
    let extra = resolve(&["formatters", "prettier", "enable"]).expect("extra formatter");
    assert_eq!(extra.value_type, ValueType::Bool);
    assert!(extra.recommended.is_none());
    assert!(resolve(&["formatters", "prettier", "command"]).is_none());
    assert!(resolve(&["formatters", "prettier"]).is_none());
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-011"))]
fn feature_unification_is_catalogued_under_build() {
    let entry = resolve(&["build", "unify"]).expect("build.unify catalog entry");
    assert_eq!(entry.value_type, ValueType::Bool);
    assert_eq!(entry.recommended, Some(DefaultValue::Bool(true)));
    assert!(resolve(&["features", "unify"]).is_none());
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-012"))]
fn catalog_covers_the_parameterized_sub_flake_table() {
    let path = resolve(&["flakes", "tests-basic", "path"]).expect("sub-flake path");
    assert_eq!(path.value_type, ValueType::Path);
    assert!(path.recommended.is_none());
    assert_eq!(
        resolve(&["flakes", "tests-basic", "packages"])
            .expect("sub-flake packages")
            .value_type,
        ValueType::Bool
    );
    assert_eq!(
        resolve(&["flakes", "tests-basic", "apps"])
            .expect("sub-flake apps")
            .value_type,
        ValueType::Bool
    );
    assert_eq!(
        resolve(&["flakes", "tests-basic", "billets"])
            .expect("sub-flake billets")
            .value_type,
        ValueType::Bool
    );
    assert_eq!(
        resolve(&["flakes", "tests-basic", "checks"])
            .expect("sub-flake checks")
            .value_type,
        ValueType::StringEnum(&["none", "individual", "aggregate"])
    );
    assert!(resolve(&["flakes", "tests-basic"]).is_none());
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-017"))]
fn catalog_covers_the_parameterized_optimization_table() {
    for (field, expected) in [("enable", ValueType::Bool), ("script", ValueType::Path)] {
        let entry = resolve(&["optimize", "my-server", field])
            .unwrap_or_else(|| panic!("optimize.{field} must be catalogued"));
        assert_eq!(entry.value_type, expected, "optimize.{field}");
    }
    assert!(resolve(&["optimize", "my-server", "targetCpu"]).is_none());
    let system = resolve(&[
        "optimize",
        "my-server",
        "systems",
        "x86_64-linux",
        "targetCpu",
    ])
    .expect("optimize.<name>.systems.<system>.targetCpu must be catalogued");
    assert_eq!(system.value_type, ValueType::String);
    for (path, expected) in [
        (["optimize", "my-server", "pgo", "enable"], ValueType::Bool),
        (
            ["optimize", "my-server", "pgo", "scope"],
            ValueType::StringEnum(&["workspace", "all"]),
        ),
        (["optimize", "my-server", "bolt", "enable"], ValueType::Bool),
        (
            ["optimize", "my-server", "bolt", "functions"],
            ValueType::Bool,
        ),
        (["optimize", "my-server", "bolt", "blocks"], ValueType::Bool),
    ] {
        let entry = resolve(&path).unwrap_or_else(|| panic!("{path:?} must be catalogued"));
        assert_eq!(entry.value_type, expected, "{path:?}");
    }
    assert!(resolve(&["optimize", "my-server", "systems", "x86_64-linux", "enable"]).is_none());
    assert!(resolve(&["optimize", "my-server", "profile"]).is_none());
    assert!(resolve(&["optimize", "my-server"]).is_none());
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-018"))]
fn optimization_keys_carry_no_recommendation() {
    for path in [
        vec!["optimize", "my-server", "enable"],
        vec!["optimize", "my-server", "script"],
        vec!["optimize", "my-server", "pgo", "enable"],
        vec!["optimize", "my-server", "pgo", "scope"],
        vec!["optimize", "my-server", "bolt", "enable"],
        vec!["optimize", "my-server", "bolt", "functions"],
        vec!["optimize", "my-server", "bolt", "blocks"],
    ] {
        let entry = resolve(&path).unwrap_or_else(|| panic!("{path:?}"));
        assert!(entry.recommended.is_none(), "{path:?}");
    }
    let system = resolve(&[
        "optimize",
        "my-server",
        "systems",
        "x86_64-linux",
        "targetCpu",
    ])
    .expect("optimize system key");
    assert!(system.recommended.is_none());
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-013"))]
fn no_other_output_family_is_catalogued_for_elevation() {
    for field in ["devShells", "formatter", "legacyPackages", "nixosModules"] {
        assert!(
            resolve(&["flakes", "tests-basic", field]).is_none(),
            "{field} must not be elevatable"
        );
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-VALIDATION-005"))]
fn formatter_ordering_keys_are_type_validated() {
    let valid: Value = toml::from_str(
        "[formatters.prettier]\nenable = true\nbefore = [\"rustfmt\"]\nafter = []\n",
    )
    .expect("toml");
    assert!(validate_raw(&valid).is_ok());

    for contents in [
        "[formatters.prettier]\nbefore = \"rustfmt\"\n",
        "[formatters.prettier]\nafter = [1]\n",
        "[formatters.prettier]\ncommand = \"prettier\"\n",
        "[formatters]\nbogus = true\n",
    ] {
        let raw: Value = toml::from_str(contents).expect("toml");
        assert!(validate_raw(&raw).is_err(), "{contents}");
    }
}

#[test]
fn validation_rejects_unknown_and_mistyped_keys() {
    for contents in [
        "[checks]\nbogus = true\n",
        "[checks]\nenable = \"yes\"\n",
        "[build.toolchain]\nlinker = \"gold\"\n",
        "[build.profile]\nrelease = { optLevel = 9 }\n",
        "[toolchain]\nrustc = \"rustc\"\n",
        "[checks]\nincludePackageChecks = true\n",
    ] {
        let raw: Value = toml::from_str(contents).expect("toml");
        assert!(validate_raw(&raw).is_err(), "{contents}");
    }
}

#[test]
fn validation_accepts_valid_build_tables() {
    let raw: Value = toml::from_str(
        "[build.toolchain]\nrustc = \"llvmPackages.lld\"\n\
             [build.profile.release]\nlto = \"fat\"\ncodegenUnits = 1\n\
             [checks]\nenable = true\n",
    )
    .expect("toml");
    assert!(validate_raw(&raw).is_ok());
}
