use crate::Diagnostic;
use toml::Value;

/// Type of a catalogued configuration value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Bool,
    /// Any integer.
    Integer,
    /// Integer greater than zero.
    PositiveInteger,
    /// Integer in `0..=max`.
    UnsignedUpTo(u64),
    String,
    /// Repository-relative or `.bloomery`-relative path encoded as a string.
    Path,
    /// nixpkgs attribute path encoded as a string.
    Package,
    StringList,
    StringEnum(&'static [&'static str]),
    Union(&'static [ValueType]),
}

/// Documented value applied to an absent key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultValue {
    Bool(bool),
    Integer(i64),
    String(&'static str),
    StringList(&'static [&'static str]),
}

impl DefaultValue {
    /// Convert the recommended default into a concrete TOML value.
    pub fn to_toml(self) -> Value {
        match self {
            Self::Bool(value) => Value::Boolean(value),
            Self::Integer(value) => Value::Integer(value),
            Self::String(value) => Value::String(value.to_owned()),
            Self::StringList(values) => Value::Array(
                values
                    .iter()
                    .map(|value| Value::String((*value).to_owned()))
                    .collect(),
            ),
        }
    }
}

/// One recognized `.bloomery/config.toml` key.
#[derive(Debug, Clone, Copy)]
pub struct CatalogEntry {
    pub path: &'static [&'static str],
    pub value_type: ValueType,
    /// Present only when omitting the key has a single fixed effective value.
    pub recommended: Option<DefaultValue>,
}

/// Increment when the catalog changes.
pub const CATALOG_VERSION: u32 = 1;

const OPT_LEVEL: &[ValueType] = &[
    ValueType::UnsignedUpTo(3),
    ValueType::StringEnum(&["0", "1", "2", "3", "s", "z"]),
];
const LTO: &[ValueType] = &[
    ValueType::Bool,
    ValueType::StringEnum(&["fat", "thin", "off", "full", "none", "yes", "no"]),
];
const PANIC: &[ValueType] = &[ValueType::StringEnum(&["unwind", "abort"])];
const STRIP: &[ValueType] = &[
    ValueType::Bool,
    ValueType::StringEnum(&["none", "debuginfo", "symbols"]),
];
const DEBUGINFO: &[ValueType] = &[
    ValueType::Bool,
    ValueType::UnsignedUpTo(2),
    ValueType::StringEnum(&[
        "0",
        "1",
        "2",
        "none",
        "line-directives-only",
        "line-tables-only",
        "limited",
        "full",
    ]),
];
const LINKER: &[ValueType] = &[ValueType::StringEnum(&["lld", "mold", "system"])];

macro_rules! profile_field {
    ($flavor:literal, $field:literal, $ty:expr) => {
        CatalogEntry {
            path: &["profile", $flavor, $field],
            value_type: $ty,
            recommended: None,
        }
    };
}

/// Every recognized configuration key, in stable path order.
pub const CATALOG: &[CatalogEntry] = &[
    // CLI-owned spec and scanner tables.
    CatalogEntry {
        path: &["specs", "dir"],
        value_type: ValueType::Path,
        recommended: Some(DefaultValue::String("specs")),
    },
    CatalogEntry {
        path: &["scanners", "rust", "enabled"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(false)),
    },
    CatalogEntry {
        path: &["scanners", "rust", "paths"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
    },
    CatalogEntry {
        path: &["scanners", "playwright", "enabled"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(false)),
    },
    CatalogEntry {
        path: &["scanners", "playwright", "paths"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
    },
    CatalogEntry {
        path: &["scanners", "playwright", "tag_prefix"],
        value_type: ValueType::String,
        recommended: Some(DefaultValue::String("@bloomery:")),
    },
    CatalogEntry {
        path: &["scanners", "nix", "enabled"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(false)),
    },
    CatalogEntry {
        path: &["scanners", "nix", "paths"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
    },
    // Nix build tables.
    CatalogEntry {
        path: &["build", "cargoToml"],
        value_type: ValueType::Path,
        recommended: Some(DefaultValue::String("Cargo.toml")),
    },
    CatalogEntry {
        path: &["build", "cargoLock"],
        value_type: ValueType::Path,
        recommended: Some(DefaultValue::String("Cargo.lock")),
    },
    CatalogEntry {
        path: &["build", "bloomeryLock"],
        value_type: ValueType::Path,
        recommended: None,
    },
    CatalogEntry {
        path: &["build", "members"],
        value_type: ValueType::StringList,
        recommended: None,
    },
    CatalogEntry {
        path: &["build", "profileName"],
        value_type: ValueType::String,
        recommended: Some(DefaultValue::String("release")),
    },
    CatalogEntry {
        path: &["build", "libPackages"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(false)),
    },
    CatalogEntry {
        path: &["build", "devPackages"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(false)),
    },
    CatalogEntry {
        path: &["toolchain", "rustc"],
        value_type: ValueType::Package,
        recommended: Some(DefaultValue::String("rustc")),
    },
    CatalogEntry {
        path: &["toolchain", "clippy"],
        value_type: ValueType::Package,
        recommended: Some(DefaultValue::String("clippy")),
    },
    CatalogEntry {
        path: &["toolchain", "cargo"],
        value_type: ValueType::Package,
        recommended: Some(DefaultValue::String("cargo")),
    },
    CatalogEntry {
        path: &["toolchain", "lld"],
        value_type: ValueType::Package,
        recommended: Some(DefaultValue::String("lld")),
    },
    CatalogEntry {
        path: &["toolchain", "mold"],
        value_type: ValueType::Package,
        recommended: Some(DefaultValue::String("mold")),
    },
    CatalogEntry {
        path: &["toolchain", "stdenv"],
        value_type: ValueType::Package,
        recommended: Some(DefaultValue::String("stdenv")),
    },
    CatalogEntry {
        path: &["toolchain", "linker"],
        value_type: ValueType::Union(LINKER),
        recommended: None,
    },
    // Compilation profile fields: optional overrides with no fixed default.
    profile_field!("release", "optLevel", ValueType::Union(OPT_LEVEL)),
    profile_field!("release", "lto", ValueType::Union(LTO)),
    profile_field!("release", "codegenUnits", ValueType::PositiveInteger),
    profile_field!("release", "panic", ValueType::Union(PANIC)),
    profile_field!("release", "strip", ValueType::Union(STRIP)),
    profile_field!("release", "linker", ValueType::String),
    profile_field!("release", "linkArgs", ValueType::StringList),
    profile_field!("release", "targetCpu", ValueType::String),
    profile_field!("release", "debuginfo", ValueType::Union(DEBUGINFO)),
    profile_field!("release", "overflowChecks", ValueType::Bool),
    profile_field!("dev", "optLevel", ValueType::Union(OPT_LEVEL)),
    profile_field!("dev", "lto", ValueType::Union(LTO)),
    profile_field!("dev", "codegenUnits", ValueType::PositiveInteger),
    profile_field!("dev", "panic", ValueType::Union(PANIC)),
    profile_field!("dev", "strip", ValueType::Union(STRIP)),
    profile_field!("dev", "linker", ValueType::String),
    profile_field!("dev", "linkArgs", ValueType::StringList),
    profile_field!("dev", "targetCpu", ValueType::String),
    profile_field!("dev", "debuginfo", ValueType::Union(DEBUGINFO)),
    profile_field!("dev", "overflowChecks", ValueType::Bool),
    CatalogEntry {
        path: &["flags", "rustc"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&["-Copt-level=3"])),
    },
    CatalogEntry {
        path: &["flags", "test"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
    },
    CatalogEntry {
        path: &["flags", "clippy"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
    },
    CatalogEntry {
        path: &["flags", "doc"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&["-Dwarnings"])),
    },
    CatalogEntry {
        path: &["flags", "doctest"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
    },
    CatalogEntry {
        path: &["devShell", "enable"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
    },
    CatalogEntry {
        path: &["devShell", "packages"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
    },
    CatalogEntry {
        path: &["devShell", "shellHook"],
        value_type: ValueType::String,
        recommended: Some(DefaultValue::String("")),
    },
    CatalogEntry {
        path: &["checks", "enable"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
    },
    CatalogEntry {
        path: &["checks", "includePackageChecks"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
    },
    CatalogEntry {
        path: &["checks", "throwOnOutOfDate"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(false)),
    },
    CatalogEntry {
        path: &["checks", "workspaceDependencies"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
    },
    CatalogEntry {
        path: &["checks", "noDefaultFeatures"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
    },
    CatalogEntry {
        path: &["features", "unify"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
    },
    CatalogEntry {
        path: &["features", "cratesIoIndex"],
        value_type: ValueType::Path,
        recommended: None,
    },
];

/// All catalog entries that carry a recommended default.
pub fn recommended_entries() -> impl Iterator<Item = &'static CatalogEntry> {
    CATALOG.iter().filter(|entry| entry.recommended.is_some())
}

/// Resolve an exact dotted path to a catalog entry.
pub fn lookup(path: &[&str]) -> Option<&'static CatalogEntry> {
    CATALOG.iter().find(|entry| entry.path == path)
}

/// Whether a table path is a strict prefix of at least one catalog entry.
fn is_known_table(path: &[&str]) -> bool {
    CATALOG
        .iter()
        .any(|entry| entry.path.len() > path.len() && entry.path.starts_with(path))
}

fn type_name(value_type: &ValueType) -> String {
    match value_type {
        ValueType::Bool => "boolean".to_owned(),
        ValueType::Integer => "integer".to_owned(),
        ValueType::PositiveInteger => "positive integer".to_owned(),
        ValueType::UnsignedUpTo(max) => format!("integer from 0 to {max}"),
        ValueType::String => "string".to_owned(),
        ValueType::Path => "path string".to_owned(),
        ValueType::Package => "nixpkgs attribute-path string".to_owned(),
        ValueType::StringList => "list of strings".to_owned(),
        ValueType::StringEnum(values) => format!("one of {}", values.join(", ")),
        ValueType::Union(types) => types.iter().map(type_name).collect::<Vec<_>>().join(" or "),
    }
}

/// Human-readable description of a schema type for usage errors.
pub fn describe_value_type(value_type: &ValueType) -> String {
    type_name(value_type)
}

fn matches(value: &Value, value_type: &ValueType) -> bool {
    match value_type {
        ValueType::Bool => value.is_bool(),
        ValueType::Integer => value.is_integer(),
        ValueType::PositiveInteger => value.as_integer().is_some_and(|value| value > 0),
        ValueType::UnsignedUpTo(max) => value
            .as_integer()
            .is_some_and(|value| value >= 0 && value as u64 <= *max),
        ValueType::String | ValueType::Path | ValueType::Package => value.is_str(),
        ValueType::StringList => value
            .as_array()
            .is_some_and(|items| items.iter().all(Value::is_str)),
        ValueType::StringEnum(values) => value
            .as_str()
            .is_some_and(|candidate| values.contains(&candidate)),
        ValueType::Union(types) => types.iter().any(|value_type| matches(value, value_type)),
    }
}

fn validate_value(path: &str, expected: &ValueType, value: &Value) -> Result<(), Diagnostic> {
    if matches(value, expected) {
        return Ok(());
    }
    Err(Diagnostic::new(
        "ConfigurationError",
        format!("{path} must be a {}", type_name(expected)),
    ))
}

fn collect_leaves<'a>(
    path: &mut Vec<&'a str>,
    value: &'a Value,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match value {
        Value::Table(table) => {
            if !is_known_table(path) {
                diagnostics.push(Diagnostic::new(
                    "ConfigurationError",
                    format!("unknown configuration table: {}", path.join(".")),
                ));
                return;
            }
            for (key, child) in table {
                path.push(key);
                collect_leaves(path, child, diagnostics);
                path.pop();
            }
        }
        _ => {
            let key = path.join(".");
            match lookup(path) {
                Some(entry) => {
                    if let Err(diagnostic) = validate_value(&key, &entry.value_type, value) {
                        diagnostics.push(diagnostic);
                    }
                }
                None => diagnostics.push(Diagnostic::new(
                    "ConfigurationError",
                    format!("unknown configuration key: {key}"),
                )),
            }
        }
    }
}

/// Validate a raw configuration tree against the schema catalog.
pub fn validate_raw(raw: &Value) -> Result<(), Diagnostic> {
    let mut diagnostics = Vec::new();
    if let Value::Table(table) = raw {
        let mut path = Vec::new();
        for (key, child) in table {
            path.push(key.as_str());
            collect_leaves(&mut path, child, &mut diagnostics);
            path.pop();
        }
    }
    match diagnostics.into_iter().next() {
        Some(diagnostic) => Err(diagnostic),
        None => Ok(()),
    }
}

/// Recommended default for a path, if any.
pub fn effective_default(path: &[&str]) -> Option<Value> {
    lookup(path)
        .and_then(|entry| entry.recommended)
        .map(DefaultValue::to_toml)
}

/// Raw value at an exact path, before defaults are applied.
pub fn raw_at<'a>(raw: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut value = raw;
    for component in path {
        value = value.as_table()?.get(*component)?;
    }
    Some(value)
}

/// Effective value for a path: the explicit raw value when present, otherwise
/// the recommended default. Explicit values always take precedence.
pub fn effective_value(raw: Option<&Value>, path: &[&str]) -> Option<Value> {
    if let Some(raw) = raw
        && let Some(value) = raw_at(raw, path)
    {
        return Some(value.clone());
    }
    effective_default(path)
}

#[cfg(test)]
mod tests {
    use super::{
        CATALOG, CATALOG_VERSION, ValueType, effective_default, effective_value, lookup,
        recommended_entries, validate_raw,
    };
    use toml::Value;

    fn path(entry: &super::CatalogEntry) -> String {
        entry.path.join(".")
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-001"))]
    fn catalog_enumerates_every_recognized_key() {
        let mut paths = CATALOG.iter().map(path).collect::<Vec<_>>();
        let count = paths.len();
        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), count, "catalog paths must be unique");
        for expected in [
            "specs.dir",
            "scanners.rust.enabled",
            "build.cargoToml",
            "toolchain.rustc",
            "profile.release.optLevel",
            "profile.dev.debuginfo",
            "flags.rustc",
            "devShell.shellHook",
            "checks.enable",
            "features.unify",
        ] {
            assert!(
                paths.iter().any(|candidate| candidate == expected),
                "{expected}"
            );
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-002"))]
    fn catalog_entries_pair_paths_with_types() {
        assert_eq!(
            lookup(&["checks", "enable"]).unwrap().value_type,
            ValueType::Bool
        );
        assert_eq!(
            lookup(&["checks", "workspaceDependencies"])
                .unwrap()
                .value_type,
            ValueType::Bool
        );
        assert_eq!(
            lookup(&["checks", "noDefaultFeatures"]).unwrap().value_type,
            ValueType::Bool
        );
        assert_eq!(
            lookup(&["scanners", "rust", "paths"]).unwrap().value_type,
            ValueType::StringList
        );
        assert_eq!(
            lookup(&["toolchain", "rustc"]).unwrap().value_type,
            ValueType::Package
        );
        assert_eq!(
            lookup(&["toolchain", "linker"]).unwrap().value_type,
            ValueType::Union(&[ValueType::StringEnum(&["lld", "mold", "system"])])
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-003"))]
    fn recommended_defaults_match_applied_defaults() {
        assert_eq!(
            effective_default(&["specs", "dir"]),
            Some(Value::String("specs".into()))
        );
        assert_eq!(
            effective_default(&["scanners", "rust", "enabled"]),
            Some(Value::Boolean(false))
        );
        assert_eq!(
            effective_default(&["flags", "rustc"]),
            Some(Value::Array(vec![Value::String("-Copt-level=3".into())]))
        );
        assert_eq!(
            effective_default(&["checks", "includePackageChecks"]),
            Some(Value::Boolean(true))
        );
        assert_eq!(
            effective_default(&["checks", "workspaceDependencies"]),
            Some(Value::Boolean(true))
        );
        assert_eq!(
            effective_default(&["checks", "noDefaultFeatures"]),
            Some(Value::Boolean(true))
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-004"))]
    fn catalog_covers_the_build_tables() {
        for prefix in [
            "build",
            "toolchain",
            "profile",
            "flags",
            "devShell",
            "checks",
            "features",
        ] {
            assert!(
                CATALOG.iter().any(|entry| entry.path[0] == prefix),
                "missing {prefix}"
            );
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-005"))]
    fn catalog_version_tracks_the_binary() {
        assert_eq!(CATALOG_VERSION, 1);
        assert!(!CATALOG.is_empty());
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-CONFIGURATION-CATALOG-006"))]
    fn optional_keys_have_no_recommendation() {
        for path in [
            "build.bloomeryLock",
            "build.members",
            "toolchain.linker",
            "features.cratesIoIndex",
            "profile.release.optLevel",
            "profile.dev.linkArgs",
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
                .any(|entry| path(entry) == "profile.release.optLevel")
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
            effective_value(Some(&raw), &["checks", "throwOnOutOfDate"]),
            Some(Value::Boolean(false))
        );
    }

    #[test]
    fn validation_rejects_unknown_and_mistyped_keys() {
        for contents in [
            "[checks]\nbogus = true\n",
            "[checks]\nenable = \"yes\"\n",
            "[toolchain]\nlinker = \"gold\"\n",
            "[profile]\nrelease = { optLevel = 9 }\n",
        ] {
            let raw: Value = toml::from_str(contents).expect("toml");
            assert!(validate_raw(&raw).is_err(), "{contents}");
        }
    }

    #[test]
    fn validation_accepts_valid_build_tables() {
        let raw: Value = toml::from_str(
            "[toolchain]\nrustc = \"llvmPackages.lld\"\n\
             [profile.release]\nlto = \"fat\"\ncodegenUnits = 1\n\
             [checks]\nenable = true\n",
        )
        .expect("toml");
        assert!(validate_raw(&raw).is_ok());
    }
}
