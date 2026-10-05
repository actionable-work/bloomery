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
    /// Documentation text rendered by `bloomery config document`.
    pub documentation: &'static str,
}

/// A resolved key, including parameterized formatter names that have no static
/// catalog entry.
#[derive(Debug, Clone)]
pub struct KeySpec {
    pub path: Vec<String>,
    pub value_type: ValueType,
    pub recommended: Option<DefaultValue>,
    pub documentation: &'static str,
}

impl KeySpec {
    /// Build a resolved key from a static catalog entry.
    pub fn from_entry(entry: &CatalogEntry) -> Self {
        Self {
            path: entry
                .path
                .iter()
                .map(|segment| (*segment).to_owned())
                .collect(),
            value_type: entry.value_type,
            recommended: entry.recommended,
            documentation: entry.documentation,
        }
    }
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

/// Built-in formatter names that the workspace ships and can order.
pub const BUILTIN_FORMATTERS: &[&str] = &[
    "alejandra",
    "deadnix",
    "rustfmt",
    "shellcheck",
    "shfmt",
    "taplo",
    "toml-sort",
    "yamlfmt",
];

macro_rules! profile_field {
    ($flavor:literal, $field:literal, $ty:expr, $doc:literal) => {
        CatalogEntry {
            path: &["profile", $flavor, $field],
            value_type: $ty,
            recommended: None,
            documentation: concat!($flavor, " profile: ", $doc),
        }
    };
}

macro_rules! formatter {
    ($name:literal, $field:literal, $ty:expr, $recommended:expr, $doc:literal) => {
        CatalogEntry {
            path: &["formatters", $name, $field],
            value_type: $ty,
            recommended: $recommended,
            documentation: $doc,
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
        documentation: "Directory inside .bloomery/ that holds the specification tree.",
    },
    CatalogEntry {
        path: &["scanners", "rust", "enabled"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(false)),
        documentation: "Enable the Rust evidence scanner.",
    },
    CatalogEntry {
        path: &["scanners", "rust", "paths"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
        documentation: "Repository-relative globs for Rust source files.",
    },
    CatalogEntry {
        path: &["scanners", "playwright", "enabled"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(false)),
        documentation: "Enable the Playwright evidence scanner.",
    },
    CatalogEntry {
        path: &["scanners", "playwright", "paths"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
        documentation: "Repository-relative globs for Playwright test files.",
    },
    CatalogEntry {
        path: &["scanners", "playwright", "tag_prefix"],
        value_type: ValueType::String,
        recommended: Some(DefaultValue::String("@bloomery:")),
        documentation: "Tag prefix that marks Bloomery metadata in Playwright tests.",
    },
    CatalogEntry {
        path: &["scanners", "nix", "enabled"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(false)),
        documentation: "Enable the static Nix evidence scanner.",
    },
    CatalogEntry {
        path: &["scanners", "nix", "paths"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
        documentation: "Repository-relative globs for Nix source files.",
    },
    // Nix build tables.
    CatalogEntry {
        path: &["build", "cargoToml"],
        value_type: ValueType::Path,
        recommended: Some(DefaultValue::String("Cargo.toml")),
        documentation: "Repository-relative path to the Cargo workspace manifest.",
    },
    CatalogEntry {
        path: &["build", "cargoLock"],
        value_type: ValueType::Path,
        recommended: Some(DefaultValue::String("Cargo.lock")),
        documentation: "Repository-relative path to Cargo.lock.",
    },
    CatalogEntry {
        path: &["build", "bloomeryLock"],
        value_type: ValueType::Path,
        recommended: None,
        documentation: "Repository-relative path to bloomery.lock; omit to auto-detect.",
    },
    CatalogEntry {
        path: &["build", "members"],
        value_type: ValueType::StringList,
        recommended: None,
        documentation: "Crate names to build; omit to build every workspace member.",
    },
    CatalogEntry {
        path: &["build", "profileName"],
        value_type: ValueType::String,
        recommended: Some(DefaultValue::String("release")),
        documentation: "Active Cargo profile name for release builds.",
    },
    CatalogEntry {
        path: &["build", "libPackages"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(false)),
        documentation: "Expose <crate>:lib packages for workspace libraries.",
    },
    CatalogEntry {
        path: &["build", "devPackages"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(false)),
        documentation: "Generate dev-profile apps for workspace binaries.",
    },
    CatalogEntry {
        path: &["toolchain", "rustc"],
        value_type: ValueType::Package,
        recommended: Some(DefaultValue::String("rustc")),
        documentation: "nixpkgs attribute path for rustc.",
    },
    CatalogEntry {
        path: &["toolchain", "clippy"],
        value_type: ValueType::Package,
        recommended: Some(DefaultValue::String("clippy")),
        documentation: "nixpkgs attribute path for clippy.",
    },
    CatalogEntry {
        path: &["toolchain", "cargo"],
        value_type: ValueType::Package,
        recommended: Some(DefaultValue::String("cargo")),
        documentation: "nixpkgs attribute path for cargo.",
    },
    CatalogEntry {
        path: &["toolchain", "lld"],
        value_type: ValueType::Package,
        recommended: Some(DefaultValue::String("lld")),
        documentation: "nixpkgs attribute path for lld.",
    },
    CatalogEntry {
        path: &["toolchain", "mold"],
        value_type: ValueType::Package,
        recommended: Some(DefaultValue::String("mold")),
        documentation: "nixpkgs attribute path for mold.",
    },
    CatalogEntry {
        path: &["toolchain", "stdenv"],
        value_type: ValueType::Package,
        recommended: Some(DefaultValue::String("stdenv")),
        documentation: "nixpkgs attribute path for the C toolchain.",
    },
    CatalogEntry {
        path: &["toolchain", "linker"],
        value_type: ValueType::Union(LINKER),
        recommended: None,
        documentation: "Linker strategy: lld, mold, system, or omit for the platform default.",
    },
    // Compilation profile fields: optional overrides with no fixed default.
    profile_field!(
        "release",
        "optLevel",
        ValueType::Union(OPT_LEVEL),
        "optimization level (0-3, s, z)"
    ),
    profile_field!(
        "release",
        "lto",
        ValueType::Union(LTO),
        "link-time optimization mode"
    ),
    profile_field!(
        "release",
        "codegenUnits",
        ValueType::PositiveInteger,
        "number of codegen units"
    ),
    profile_field!(
        "release",
        "panic",
        ValueType::Union(PANIC),
        "panic strategy"
    ),
    profile_field!(
        "release",
        "strip",
        ValueType::Union(STRIP),
        "debug symbol stripping"
    ),
    profile_field!("release", "linker", ValueType::String, "linker driver"),
    profile_field!(
        "release",
        "linkArgs",
        ValueType::StringList,
        "extra linker arguments"
    ),
    profile_field!("release", "targetCpu", ValueType::String, "target CPU"),
    profile_field!(
        "release",
        "debuginfo",
        ValueType::Union(DEBUGINFO),
        "debug info level"
    ),
    profile_field!(
        "release",
        "overflowChecks",
        ValueType::Bool,
        "enable integer overflow checks"
    ),
    profile_field!(
        "dev",
        "optLevel",
        ValueType::Union(OPT_LEVEL),
        "optimization level (0-3, s, z)"
    ),
    profile_field!(
        "dev",
        "lto",
        ValueType::Union(LTO),
        "link-time optimization mode"
    ),
    profile_field!(
        "dev",
        "codegenUnits",
        ValueType::PositiveInteger,
        "number of codegen units"
    ),
    profile_field!("dev", "panic", ValueType::Union(PANIC), "panic strategy"),
    profile_field!(
        "dev",
        "strip",
        ValueType::Union(STRIP),
        "debug symbol stripping"
    ),
    profile_field!("dev", "linker", ValueType::String, "linker driver"),
    profile_field!(
        "dev",
        "linkArgs",
        ValueType::StringList,
        "extra linker arguments"
    ),
    profile_field!("dev", "targetCpu", ValueType::String, "target CPU"),
    profile_field!(
        "dev",
        "debuginfo",
        ValueType::Union(DEBUGINFO),
        "debug info level"
    ),
    profile_field!(
        "dev",
        "overflowChecks",
        ValueType::Bool,
        "enable integer overflow checks"
    ),
    CatalogEntry {
        path: &["flags", "rustc"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&["-Copt-level=3"])),
        documentation: "Extra rustc flags for release compilation.",
    },
    CatalogEntry {
        path: &["flags", "test"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
        documentation: "Extra rustc flags for test compilation.",
    },
    CatalogEntry {
        path: &["flags", "clippy"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
        documentation: "Extra rustc flags for clippy runs.",
    },
    CatalogEntry {
        path: &["flags", "doc"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&["-Dwarnings"])),
        documentation: "Extra rustdoc flags for documentation builds.",
    },
    CatalogEntry {
        path: &["flags", "doctest"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
        documentation: "Extra rustdoc flags for doctests.",
    },
    CatalogEntry {
        path: &["devShell", "enable"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
        documentation: "Generate the default development shell.",
    },
    CatalogEntry {
        path: &["devShell", "packages"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
        documentation: "Additional nixpkgs packages in the development shell.",
    },
    CatalogEntry {
        path: &["devShell", "shellHook"],
        value_type: ValueType::String,
        recommended: Some(DefaultValue::String("")),
        documentation: "Shell hook run when entering the development shell.",
    },
    CatalogEntry {
        path: &["checks", "enable"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
        documentation: "Generate workspace checks.",
    },
    CatalogEntry {
        path: &["checks", "includePackageChecks"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
        documentation: "Include binary and library build checks.",
    },
    CatalogEntry {
        path: &["checks", "throwOnOutOfDate"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(false)),
        documentation: "Fail evaluation when bloomery.lock is out of date.",
    },
    CatalogEntry {
        path: &["checks", "workspaceDependencies"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
        documentation: "Validate workspace dependency declarations.",
    },
    CatalogEntry {
        path: &["checks", "noDefaultFeatures"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
        documentation: "Require default features to be disabled on workspace dependencies.",
    },
    CatalogEntry {
        path: &["features", "unify"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
        documentation: "Unify workspace features like Cargo's resolver.",
    },
    CatalogEntry {
        path: &["features", "cratesIoIndex"],
        value_type: ValueType::Path,
        recommended: None,
        documentation: "Repository-relative path to a local crates.io index.",
    },
    // Built-in formatters. Extra formatter names are addressable through the
    // parameterized family but have no recommendation.
    formatter!(
        "alejandra",
        "enable",
        ValueType::Bool,
        Some(DefaultValue::Bool(true)),
        "Enable the Alejandra Nix formatter."
    ),
    formatter!(
        "alejandra",
        "before",
        ValueType::StringList,
        Some(DefaultValue::StringList(&[])),
        "Formatters that must run after the Alejandra formatter."
    ),
    formatter!(
        "alejandra",
        "after",
        ValueType::StringList,
        Some(DefaultValue::StringList(&[])),
        "Formatters that must run before the Alejandra formatter."
    ),
    formatter!(
        "deadnix",
        "enable",
        ValueType::Bool,
        Some(DefaultValue::Bool(true)),
        "Enable the deadnix Nix formatter."
    ),
    formatter!(
        "deadnix",
        "before",
        ValueType::StringList,
        Some(DefaultValue::StringList(&[])),
        "Formatters that must run after the deadnix formatter."
    ),
    formatter!(
        "deadnix",
        "after",
        ValueType::StringList,
        Some(DefaultValue::StringList(&[])),
        "Formatters that must run before the deadnix formatter."
    ),
    formatter!(
        "rustfmt",
        "enable",
        ValueType::Bool,
        Some(DefaultValue::Bool(true)),
        "Enable the rustfmt formatter."
    ),
    formatter!(
        "rustfmt",
        "before",
        ValueType::StringList,
        Some(DefaultValue::StringList(&[])),
        "Formatters that must run after the rustfmt formatter."
    ),
    formatter!(
        "rustfmt",
        "after",
        ValueType::StringList,
        Some(DefaultValue::StringList(&[])),
        "Formatters that must run before the rustfmt formatter."
    ),
    formatter!(
        "shellcheck",
        "enable",
        ValueType::Bool,
        Some(DefaultValue::Bool(true)),
        "Enable the shellcheck formatter."
    ),
    formatter!(
        "shellcheck",
        "before",
        ValueType::StringList,
        Some(DefaultValue::StringList(&[])),
        "Formatters that must run after the shellcheck formatter."
    ),
    formatter!(
        "shellcheck",
        "after",
        ValueType::StringList,
        Some(DefaultValue::StringList(&[])),
        "Formatters that must run before the shellcheck formatter."
    ),
    formatter!(
        "shfmt",
        "enable",
        ValueType::Bool,
        Some(DefaultValue::Bool(true)),
        "Enable the shfmt shell formatter."
    ),
    formatter!(
        "shfmt",
        "before",
        ValueType::StringList,
        Some(DefaultValue::StringList(&[])),
        "Formatters that must run after the shfmt formatter."
    ),
    formatter!(
        "shfmt",
        "after",
        ValueType::StringList,
        Some(DefaultValue::StringList(&[])),
        "Formatters that must run before the shfmt formatter."
    ),
    formatter!(
        "taplo",
        "enable",
        ValueType::Bool,
        Some(DefaultValue::Bool(true)),
        "Enable the taplo TOML formatter."
    ),
    formatter!(
        "taplo",
        "before",
        ValueType::StringList,
        Some(DefaultValue::StringList(&[])),
        "Formatters that must run after the taplo formatter."
    ),
    formatter!(
        "taplo",
        "after",
        ValueType::StringList,
        Some(DefaultValue::StringList(&[])),
        "Formatters that must run before the taplo formatter."
    ),
    formatter!(
        "toml-sort",
        "enable",
        ValueType::Bool,
        Some(DefaultValue::Bool(true)),
        "Enable the toml-sort TOML formatter."
    ),
    formatter!(
        "toml-sort",
        "before",
        ValueType::StringList,
        Some(DefaultValue::StringList(&[])),
        "Formatters that must run after the toml-sort formatter."
    ),
    formatter!(
        "toml-sort",
        "after",
        ValueType::StringList,
        Some(DefaultValue::StringList(&[])),
        "Formatters that must run before the toml-sort formatter."
    ),
    formatter!(
        "yamlfmt",
        "enable",
        ValueType::Bool,
        Some(DefaultValue::Bool(true)),
        "Enable the yamlfmt formatter."
    ),
    formatter!(
        "yamlfmt",
        "before",
        ValueType::StringList,
        Some(DefaultValue::StringList(&[])),
        "Formatters that must run after the yamlfmt formatter."
    ),
    formatter!(
        "yamlfmt",
        "after",
        ValueType::StringList,
        Some(DefaultValue::StringList(&[])),
        "Formatters that must run before the yamlfmt formatter."
    ),
];

/// All catalog entries that carry a recommended default.
pub fn recommended_entries() -> impl Iterator<Item = &'static CatalogEntry> {
    CATALOG.iter().filter(|entry| entry.recommended.is_some())
}

/// Resolve an exact dotted path to a static catalog entry.
pub fn lookup(path: &[&str]) -> Option<&'static CatalogEntry> {
    CATALOG.iter().find(|entry| entry.path == path)
}

/// Resolve a dotted path, including parameterized formatter names that have no
/// static catalog entry.
pub fn resolve(path: &[&str]) -> Option<KeySpec> {
    if let Some(entry) = lookup(path) {
        return Some(KeySpec::from_entry(entry));
    }
    formatter_key(path)
}

fn formatter_key(path: &[&str]) -> Option<KeySpec> {
    let [table, _name, field] = path else {
        return None;
    };
    if *table != "formatters" {
        return None;
    }
    let (value_type, documentation) = match *field {
        "enable" => (ValueType::Bool, "Enable or disable this formatter."),
        "before" => (
            ValueType::StringList,
            "Formatters that must run after this formatter.",
        ),
        "after" => (
            ValueType::StringList,
            "Formatters that must run before this formatter.",
        ),
        _ => return None,
    };
    Some(KeySpec {
        path: path.iter().map(|segment| (*segment).to_owned()).collect(),
        value_type,
        recommended: None,
        documentation,
    })
}

/// Whether a path is a recognized table that may contain catalogued leaves.
fn is_known_table(path: &[&str]) -> bool {
    if path == ["formatters"] || (path.len() == 2 && path[0] == "formatters") {
        return true;
    }
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
            match resolve(path) {
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
        BUILTIN_FORMATTERS, CATALOG, CATALOG_VERSION, ValueType, effective_default,
        effective_value, lookup, recommended_entries, resolve, validate_raw,
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
            "formatters.toml-sort.before",
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
            "formatters",
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
