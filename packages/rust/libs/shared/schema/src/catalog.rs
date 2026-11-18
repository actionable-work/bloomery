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
    /// A finite integer or floating point number.
    Number,
    /// Repository-relative or `.bloomery`-relative path encoded as a string.
    Path,
    /// nixpkgs attribute path encoded as a string.
    Package,
    StringList,
    StringEnum(&'static [&'static str]),
    Union(&'static [ValueType]),
}

/// Documented value applied to an absent key.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DefaultValue {
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(&'static str),
    StringList(&'static [&'static str]),
}

impl DefaultValue {
    /// Convert the recommended default into a concrete TOML value.
    pub fn to_toml(self) -> Value {
        match self {
            Self::Bool(value) => Value::Boolean(value),
            Self::Integer(value) => Value::Integer(value),
            Self::Float(value) => Value::Float(value),
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
    /// Value recommended by `config upgrade`; may differ from omission behavior.
    pub recommended: Option<DefaultValue>,
    /// Documentation text rendered by `bloomery config document`.
    pub documentation: &'static str,
}

impl CatalogEntry {
    /// Value applied when this key is omitted. Check controls are opt-in even
    /// though config upgrade recommends explicitly enabling them.
    pub fn effective_default(&self) -> Option<DefaultValue> {
        if is_check_control(self.path) {
            Some(DefaultValue::Bool(false))
        } else {
            self.recommended
        }
    }
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
pub const CATALOG_VERSION: u32 = 15;

fn is_check_control(path: &[&str]) -> bool {
    matches!(
        path,
        ["checks", "enable"]
            | ["checks", "crates", "enable"]
            | ["checks", "packages", "enable"]
            | ["checks", "lockFreshness", "enable"]
            | ["checks", "workspaceDependencies", "enable"]
            | ["checks", "noDefaultFeatures", "enable"]
            | ["checks", "vulnerabilities", "enable"]
            | ["checks", "bans", "enable"]
            | ["checks", "licenses", "enable"]
            | ["checks", "specifications", "enable"]
            | ["checks", "codeHealth", "enable"]
            | ["checks", "clippySuppressions", "enable"]
    )
}

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
            path: &["build", "profile", $flavor, $field],
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
        path: &["specs", "enable"],
        value_type: ValueType::Bool,
        recommended: None,
        documentation: "Override specification-management auto-detection; absent enables it exactly when the specification tree exists.",
    },
    CatalogEntry {
        path: &["specs", "dir"],
        value_type: ValueType::Path,
        recommended: Some(DefaultValue::String("specs")),
        documentation: "Directory inside .bloomery/ that holds the specification tree.",
    },
    CatalogEntry {
        path: &["scanners", "rust", "enabled"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
        documentation: "Enable the Rust evidence scanner.",
    },
    CatalogEntry {
        path: &["scanners", "rust", "paths"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&["packages/rust/**/*.rs"])),
        documentation: "Repository-relative globs for Rust source files.",
    },
    CatalogEntry {
        path: &["scanners", "playwright", "enabled"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
        documentation: "Enable the Playwright evidence scanner.",
    },
    CatalogEntry {
        path: &["scanners", "playwright", "paths"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[
            "packages/playwright/**/*.spec.ts",
            "packages/playwright/**/*.test.ts",
        ])),
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
        recommended: Some(DefaultValue::Bool(true)),
        documentation: "Enable the static Nix evidence scanner.",
    },
    CatalogEntry {
        path: &["scanners", "nix", "paths"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[
            "*.nix",
            "lib/**/*.nix",
            "nix/**/*.nix",
            "tests/**/*.nix",
        ])),
        documentation: "Repository-relative globs for Nix source files.",
    },
    CatalogEntry {
        path: &["scanners", "nix", "testPaths"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&["**/*.test.nix"])),
        documentation: "Repository-relative globs for Nix test files discovered as candidates.",
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
        path: &["build", "toolchain", "rustc"],
        value_type: ValueType::Package,
        recommended: Some(DefaultValue::String("rustc")),
        documentation: "nixpkgs attribute path for rustc.",
    },
    CatalogEntry {
        path: &["build", "toolchain", "clippy"],
        value_type: ValueType::Package,
        recommended: Some(DefaultValue::String("clippy")),
        documentation: "nixpkgs attribute path for clippy.",
    },
    CatalogEntry {
        path: &["build", "toolchain", "cargo"],
        value_type: ValueType::Package,
        recommended: Some(DefaultValue::String("cargo")),
        documentation: "nixpkgs attribute path for cargo.",
    },
    CatalogEntry {
        path: &["build", "toolchain", "lld"],
        value_type: ValueType::Package,
        recommended: Some(DefaultValue::String("lld")),
        documentation: "nixpkgs attribute path for lld.",
    },
    CatalogEntry {
        path: &["build", "toolchain", "mold"],
        value_type: ValueType::Package,
        recommended: Some(DefaultValue::String("mold")),
        documentation: "nixpkgs attribute path for mold.",
    },
    CatalogEntry {
        path: &["build", "toolchain", "stdenv"],
        value_type: ValueType::Package,
        recommended: Some(DefaultValue::String("stdenv")),
        documentation: "nixpkgs attribute path for the C toolchain.",
    },
    CatalogEntry {
        path: &["build", "toolchain", "linker"],
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
        path: &["build", "flags", "rustc"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&["-Copt-level=3"])),
        documentation: "Extra rustc flags for release compilation.",
    },
    CatalogEntry {
        path: &["build", "flags", "test"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
        documentation: "Extra rustc flags for test compilation.",
    },
    CatalogEntry {
        path: &["build", "flags", "clippy"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
        documentation: "Extra rustc flags for clippy runs.",
    },
    CatalogEntry {
        path: &["build", "flags", "doc"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&["-Dwarnings"])),
        documentation: "Extra rustdoc flags for documentation builds.",
    },
    CatalogEntry {
        path: &["build", "flags", "doctest"],
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
        path: &["checks", "packages", "enable"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
        documentation: "Include binary and library build checks.",
    },
    CatalogEntry {
        path: &["checks", "crates", "enable"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
        documentation: "Generate per-crate type-check entries.",
    },
    CatalogEntry {
        path: &["checks", "lockFreshness", "enable"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
        documentation: "Fail evaluation when bloomery.lock is out of date.",
    },
    CatalogEntry {
        path: &["checks", "workspaceDependencies", "enable"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
        documentation: "Validate workspace dependency declarations.",
    },
    CatalogEntry {
        path: &["checks", "noDefaultFeatures", "enable"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
        documentation: "Require default features to be disabled on workspace dependencies.",
    },
    CatalogEntry {
        path: &["checks", "vulnerabilities", "enable"],
        value_type: ValueType::Bool,
        recommended: None,
        documentation: "Scan selected member dependencies against the supplied RustSec database.",
    },
    CatalogEntry {
        path: &["checks", "vulnerabilities", "ignoreAdvisories"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
        documentation: "RustSec advisory IDs suppressed by vulnerability checks.",
    },
    CatalogEntry {
        path: &["checks", "vulnerabilities", "informational", "unmaintained"],
        value_type: ValueType::StringEnum(&["warn", "deny", "ignore"]),
        recommended: Some(DefaultValue::String("warn")),
        documentation: "Outcome for unmaintained-crate advisories.",
    },
    CatalogEntry {
        path: &["checks", "vulnerabilities", "informational", "unsound"],
        value_type: ValueType::StringEnum(&["warn", "deny", "ignore"]),
        recommended: Some(DefaultValue::String("warn")),
        documentation: "Outcome for unsoundness advisories.",
    },
    CatalogEntry {
        path: &["checks", "vulnerabilities", "informational", "notice"],
        value_type: ValueType::StringEnum(&["warn", "deny", "ignore"]),
        recommended: Some(DefaultValue::String("warn")),
        documentation: "Outcome for other informational advisories.",
    },
    CatalogEntry {
        path: &["checks", "vulnerabilities", "informational", "unknown"],
        value_type: ValueType::StringEnum(&["warn", "deny", "ignore"]),
        recommended: Some(DefaultValue::String("warn")),
        documentation: "Outcome for unrecognized advisory categories.",
    },
    CatalogEntry {
        path: &["checks", "bans", "enable"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
        documentation: "Check package allowlists, denylists, and exceptions.",
    },
    CatalogEntry {
        path: &["checks", "bans", "allow"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
        documentation: "Package selectors permitted by the package ban check.",
    },
    CatalogEntry {
        path: &["checks", "bans", "deny"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
        documentation: "Package selectors rejected by the package ban check.",
    },
    CatalogEntry {
        path: &["checks", "bans", "exceptions"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
        documentation: "Package selectors exempt from package ban rules.",
    },
    CatalogEntry {
        path: &["checks", "licenses", "enable"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
        documentation: "Check dependency license expressions against compatibility policy.",
    },
    CatalogEntry {
        path: &["checks", "licenses", "allow"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
        documentation: "SPDX license IDs or license-with-exception pairs allowed by the license check.",
    },
    CatalogEntry {
        path: &["checks", "licenses", "deny"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
        documentation: "SPDX license IDs or license-with-exception pairs rejected by the license check.",
    },
    CatalogEntry {
        path: &["checks", "licenses", "exceptions"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
        documentation: "Package selectors exempt from global license allow and deny rules.",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "enable"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
        documentation: "Enable the source-based code health check.",
    },
    CatalogEntry {
        path: &["checks", "specifications", "enable"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
        documentation: "Enable static specification validation checks.",
    },
    CatalogEntry {
        path: &["checks", "specifications", "maxTestsPerRequirement"],
        value_type: ValueType::PositiveInteger,
        recommended: Some(DefaultValue::Integer(5)),
        documentation: "Maximum distinct authored validating sites per automated requirement.",
    },
    CatalogEntry {
        path: &["checks", "specifications", "maxRequirementsPerTest"],
        value_type: ValueType::PositiveInteger,
        recommended: Some(DefaultValue::Integer(5)),
        documentation: "Maximum distinct automated requirement IDs per authored validating site.",
    },
    CatalogEntry {
        path: &["checks", "specifications", "untiedTests"],
        value_type: ValueType::StringEnum(&["off", "report", "deny"]),
        recommended: Some(DefaultValue::String("report")),
        documentation: "Untied-test policy: off, advisory reporting, or a failing budget.",
    },
    CatalogEntry {
        path: &["checks", "specifications", "maxUntiedTests"],
        value_type: ValueType::UnsignedUpTo(i64::MAX as u64),
        recommended: Some(DefaultValue::Integer(0)),
        documentation: "Maximum nonexempt untied sites in deny mode.",
    },
    CatalogEntry {
        path: &["checks", "specifications", "requireCrateOwnership"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(false)),
        documentation: "Require an ownership mapping or exclusion for every selected workspace crate.",
    },
    CatalogEntry {
        path: &["checks", "clippySuppressions", "enable"],
        value_type: ValueType::Bool,
        recommended: Some(DefaultValue::Bool(true)),
        documentation: "Enable the inline Clippy suppression policy check.",
    },
    CatalogEntry {
        path: &["checks", "clippySuppressions", "allow"],
        value_type: ValueType::StringList,
        recommended: Some(DefaultValue::StringList(&[])),
        documentation: "Exact Clippy lint names permitted in inline allow attributes.",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "baseline"],
        value_type: ValueType::Path,
        recommended: Some(DefaultValue::String(".bloomery/code-health-summary.json")),
        documentation: "Repository-relative reviewed code health snapshot.",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "minCrateScore"],
        value_type: ValueType::Number,
        recommended: Some(DefaultValue::Integer(80)),
        documentation: "Minimum passing code health score for each crate (0–100).",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "minWorkspaceScore"],
        value_type: ValueType::Number,
        recommended: Some(DefaultValue::Integer(80)),
        documentation: "Minimum passing aggregate code health score (0–100).",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "maxRegression"],
        value_type: ValueType::Number,
        recommended: Some(DefaultValue::Integer(2)),
        documentation: "Maximum allowed decrease from a reviewed score (0–100 points).",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "cyclomaticLimit"],
        value_type: ValueType::PositiveInteger,
        recommended: Some(DefaultValue::Integer(10)),
        documentation: "Soft cyclomatic complexity limit used by code health scoring.",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "functionTokensLimit"],
        value_type: ValueType::PositiveInteger,
        recommended: Some(DefaultValue::Integer(250)),
        documentation: "Soft callable body token limit used by code health scoring.",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "nestingLimit"],
        value_type: ValueType::PositiveInteger,
        recommended: Some(DefaultValue::Integer(4)),
        documentation: "Soft branch nesting limit used by code health scoring.",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "crateSlocLimit"],
        value_type: ValueType::PositiveInteger,
        recommended: Some(DefaultValue::Integer(1_500)),
        documentation: "Soft production SLOC limit per crate used by code health scoring.",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "duplicationFreeRatio"],
        value_type: ValueType::Number,
        recommended: Some(DefaultValue::Float(0.02)),
        documentation: "Duplicate body-token fraction without a scoring penalty (0–1).",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "duplicateMinTokens"],
        value_type: ValueType::PositiveInteger,
        recommended: Some(DefaultValue::Integer(64)),
        documentation: "Minimum exact token window considered a duplicate (at least 16).",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "maxCyclomatic"],
        value_type: ValueType::PositiveInteger,
        recommended: Some(DefaultValue::Integer(40)),
        documentation: "Hard per-callable cyclomatic complexity limit.",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "maxFunctionTokens"],
        value_type: ValueType::PositiveInteger,
        recommended: Some(DefaultValue::Integer(1000)),
        documentation: "Hard per-callable body token limit.",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "maxNesting"],
        value_type: ValueType::PositiveInteger,
        recommended: Some(DefaultValue::Integer(8)),
        documentation: "Hard per-callable branch nesting limit.",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "maxCrateSloc"],
        value_type: ValueType::PositiveInteger,
        recommended: Some(DefaultValue::Integer(5_000)),
        documentation: "Hard production SLOC limit per crate.",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "weights", "complexity"],
        value_type: ValueType::Number,
        recommended: Some(DefaultValue::Integer(25)),
        documentation: "Relative code health score weight for cyclomatic complexity.",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "weights", "size"],
        value_type: ValueType::Number,
        recommended: Some(DefaultValue::Integer(20)),
        documentation: "Relative code health score weight for callable body size.",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "weights", "nesting"],
        value_type: ValueType::Number,
        recommended: Some(DefaultValue::Integer(10)),
        documentation: "Relative code health score weight for branch nesting.",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "weights", "duplication"],
        value_type: ValueType::Number,
        recommended: Some(DefaultValue::Integer(15)),
        documentation: "Relative code health score weight for duplicated code.",
    },
    CatalogEntry {
        path: &["checks", "codeHealth", "weights", "crateSize"],
        value_type: ValueType::Number,
        recommended: Some(DefaultValue::Integer(30)),
        documentation: "Relative code health score weight for production SLOC per crate.",
    },
    CatalogEntry {
        path: &["build", "unify"],
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

/// Resolve a dotted path, including parameterized formatter and sub-flake
/// names that have no static catalog entry.
pub fn resolve(path: &[&str]) -> Option<KeySpec> {
    if let Some(entry) = lookup(path) {
        return Some(KeySpec::from_entry(entry));
    }
    license_override_key(path)
        .or_else(|| specification_key(path))
        .or_else(|| formatter_key(path))
        .or_else(|| flake_key(path))
        .or_else(|| crate_exception_key(path))
        .or_else(|| optimize_key(path))
}

fn crate_exception_key(path: &[&str]) -> Option<KeySpec> {
    let ["checks", "codeHealth", "crateExceptions", _manifest, field] = path else {
        return None;
    };
    if !matches!(
        *field,
        "maxCyclomatic" | "maxFunctionTokens" | "maxNesting" | "maxCrateSloc"
    ) {
        return None;
    }
    Some(KeySpec {
        path: path.iter().map(|segment| (*segment).to_owned()).collect(),
        value_type: ValueType::PositiveInteger,
        recommended: None,
        documentation: "Hard code health limit override for one workspace crate.",
    })
}

fn specification_key(path: &[&str]) -> Option<KeySpec> {
    let ["checks", "specifications", family, _key, field] = path else {
        return None;
    };
    let (value_type, documentation) = match (*family, *field) {
        ("requirementExceptions", "maxTests") => (
            ValueType::PositiveInteger,
            "Validating-site limit for this requirement.",
        ),
        ("requirementExceptions" | "untiedExceptions" | "testExceptions" | "crates", "reason") => (
            ValueType::String,
            "Reason for this specification policy exception or ownership.",
        ),
        ("untiedExceptions", "name") => (
            ValueType::String,
            "Exact test name restricted by this untied exception.",
        ),
        ("testExceptions", "path") => (
            ValueType::Path,
            "Repository-relative path glob for this test breadth exception.",
        ),
        ("testExceptions", "name") => (
            ValueType::String,
            "Exact test name restricted by this breadth exception.",
        ),
        ("testExceptions", "maxRequirements") => (
            ValueType::PositiveInteger,
            "Requirement ID limit for matching validating sites.",
        ),
        ("crates", "features") => (
            ValueType::StringList,
            "Feature identifiers owned by this crate.",
        ),
        ("crates", "minRequirements" | "minLinkedTests") => (
            ValueType::UnsignedUpTo(i64::MAX as u64),
            "Minimum distinct owned requirements or validating sites.",
        ),
        ("crates", "exclude") => (
            ValueType::Bool,
            "Exclude this crate from ownership gates with a reason.",
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

/// Parameterized `[optimize.<bin>]` keys. Binary names are repository-defined,
/// so these keys carry no recommended default. Each named entry may also carry
/// `pgo` and `bolt` stage tables and a `systems` table keyed by system name.
fn optimize_key(path: &[&str]) -> Option<KeySpec> {
    let (value_type, documentation) = match path {
        ["optimize", _name, "enable"] => (
            ValueType::Bool,
            "Enable the optimized build for this binary.",
        ),
        ["optimize", _name, "script"] => (
            ValueType::Path,
            "Repository-relative training script for the optimized build.",
        ),
        ["optimize", _name, "pgo", "enable"] => (
            ValueType::Bool,
            "Enable profile-guided optimization for this binary.",
        ),
        ["optimize", _name, "pgo", "scope"] => (
            ValueType::StringEnum(&["workspace", "all"]),
            "PGO instrumentation scope: workspace or all.",
        ),
        ["optimize", _name, "bolt", "enable"] => (
            ValueType::Bool,
            "Enable BOLT layout optimization for this binary.",
        ),
        ["optimize", _name, "bolt", "functions"] => (
            ValueType::Bool,
            "Reorder functions with BOLT; requires relocations.",
        ),
        ["optimize", _name, "bolt", "blocks"] => {
            (ValueType::Bool, "Reorder basic blocks with BOLT.")
        }
        ["optimize", _name, "systems", _system, "targetCpu"] => (
            ValueType::String,
            "Target CPU override for this system's optimized build.",
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

fn flake_key(path: &[&str]) -> Option<KeySpec> {
    let [table, _name, field] = path else {
        return None;
    };
    if *table != "flakes" {
        return None;
    }
    let (value_type, documentation) = match *field {
        "path" => (
            ValueType::Path,
            "Repository-relative path to a sub-flake directory.",
        ),
        "packages" => (
            ValueType::Bool,
            "Elevate the sub-flake's packages into the main flake.",
        ),
        "apps" => (
            ValueType::Bool,
            "Elevate the sub-flake's apps into the main flake.",
        ),
        "checks" => (
            ValueType::StringEnum(&["none", "individual", "aggregate"]),
            "Sub-flake check elevation: none, individual, or aggregate.",
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

fn license_override_key(path: &[&str]) -> Option<KeySpec> {
    let ["checks", "licenses", "overrides", selector] = path else {
        return None;
    };
    if !valid_package_selector(selector) {
        return None;
    }
    Some(KeySpec {
        path: path.iter().map(|segment| (*segment).to_owned()).collect(),
        value_type: ValueType::String,
        recommended: None,
        documentation: "SPDX license expression overriding Cargo metadata for this package selector.",
    })
}

fn valid_package_selector(selector: &str) -> bool {
    let mut parts = selector.split('@');
    let Some(name) = parts.next() else {
        return false;
    };
    let valid_name = !name.is_empty()
        && name.as_bytes()[0].is_ascii_alphanumeric()
        && name.bytes().all(|character| {
            character.is_ascii_alphanumeric() || character == b'-' || character == b'_'
        });
    match parts.next() {
        None => valid_name,
        Some(version) => valid_name && parts.next().is_none() && valid_exact_version(version),
    }
}

fn valid_exact_version(version: &str) -> bool {
    let core = version.split(['-', '+']).next().unwrap_or_default();
    let mut components = core.split('.');
    let numeric = components.clone().count() == 3
        && components.all(|component| {
            !component.is_empty()
                && component
                    .bytes()
                    .all(|character| character.is_ascii_digit())
        });
    let suffix = version.bytes().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, b'.' | b'-' | b'+')
    });
    numeric && suffix
}

fn valid_advisory_id(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("RUSTSEC-") else {
        return false;
    };
    let Some((year, issue)) = rest.split_once('-') else {
        return false;
    };
    year.len() == 4
        && issue.len() == 4
        && year.bytes().all(|character| character.is_ascii_digit())
        && issue.bytes().all(|character| character.is_ascii_digit())
}

/// Whether a path is a recognized table that may contain catalogued leaves.
fn is_known_table(path: &[&str]) -> bool {
    if (path.len() == 3 || path.len() == 4)
        && path[..2] == ["checks", "specifications"]
        && matches!(
            path[2],
            "requirementExceptions" | "untiedExceptions" | "testExceptions" | "crates"
        )
    {
        return true;
    }
    if path == ["checks", "codeHealth", "crateExceptions"]
        || (path.len() == 4 && path[..3] == ["checks", "codeHealth", "crateExceptions"])
    {
        return true;
    }
    if path == ["formatters"] || (path.len() == 2 && path[0] == "formatters") {
        return true;
    }
    if path == ["flakes"] || (path.len() == 2 && path[0] == "flakes") {
        return true;
    }
    if path == ["checks", "licenses", "overrides"] {
        return true;
    }
    if path == ["optimize"] || (path.len() == 2 && path[0] == "optimize") {
        return true;
    }
    if path.len() == 3
        && path[0] == "optimize"
        && (path[2] == "systems" || path[2] == "pgo" || path[2] == "bolt")
    {
        return true;
    }
    if path.len() == 4 && path[0] == "optimize" && path[2] == "systems" {
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
        ValueType::Number => "number".to_owned(),
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
        ValueType::Number => {
            value.as_float().is_some_and(f64::is_finite) || value.as_integer().is_some()
        }
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
            if key == "version" {
                continue;
            }
            path.push(key.as_str());
            collect_leaves(&mut path, child, &mut diagnostics);
            path.pop();
        }
    }
    if let Some(allow) =
        raw_at(raw, &["checks", "clippySuppressions", "allow"]).and_then(Value::as_array)
    {
        let mut seen = std::collections::BTreeSet::new();
        for lint in allow.iter().filter_map(Value::as_str) {
            if !valid_clippy_lint_name(lint) {
                diagnostics.push(Diagnostic::new(
                    "ConfigurationError",
                    format!(
                        "checks.clippySuppressions.allow entries must be qualified Clippy lint names; found '{lint}'"
                    ),
                ));
            } else if !seen.insert(lint) {
                diagnostics.push(Diagnostic::new(
                    "ConfigurationError",
                    format!("checks.clippySuppressions.allow contains duplicate lint '{lint}'"),
                ));
            }
        }
    }
    for path in [
        &["checks", "bans", "allow"][..],
        &["checks", "bans", "deny"][..],
        &["checks", "bans", "exceptions"][..],
        &["checks", "licenses", "exceptions"][..],
    ] {
        if let Some(values) = raw_at(raw, path).and_then(Value::as_array) {
            for selector in values.iter().filter_map(Value::as_str) {
                if !valid_package_selector(selector) {
                    diagnostics.push(Diagnostic::new(
                        "ConfigurationError",
                        format!(
                            "{} contains invalid package selector '{selector}'",
                            path.join(".")
                        ),
                    ));
                }
            }
        }
    }
    if let Some(values) =
        raw_at(raw, &["checks", "vulnerabilities", "ignoreAdvisories"]).and_then(Value::as_array)
    {
        for advisory in values.iter().filter_map(Value::as_str) {
            if !valid_advisory_id(advisory) {
                diagnostics.push(Diagnostic::new(
                    "ConfigurationError",
                    format!(
                        "checks.vulnerabilities.ignoreAdvisories contains invalid RustSec advisory ID '{advisory}'"
                    ),
                ));
            }
        }
    }
    match diagnostics.into_iter().next() {
        Some(diagnostic) => Err(diagnostic),
        None => {
            crate::specifications::SpecificationPolicy::from_value(raw_at(
                raw,
                &["checks", "specifications"],
            ))?;
            Ok(())
        }
    }
}

fn valid_clippy_lint_name(name: &str) -> bool {
    let Some(lint) = name.strip_prefix("clippy::") else {
        return false;
    };
    let mut chars = lint.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

/// Recommended default for a path, if any.
/// Catalog default string at `path`; panics when the catalog has none, which
/// is a catalog definition error.
pub fn default_string(path: &[&str]) -> String {
    match lookup(path).and_then(CatalogEntry::effective_default) {
        Some(DefaultValue::String(value)) => value.to_owned(),
        _ => panic!("catalog has no string default for {}", path.join(".")),
    }
}

/// Catalog default string list at `path`; panics when the catalog has none,
/// which is a catalog definition error.
pub fn default_string_list(path: &[&str]) -> Vec<String> {
    match lookup(path).and_then(CatalogEntry::effective_default) {
        Some(DefaultValue::StringList(values)) => {
            values.iter().map(|value| (*value).to_owned()).collect()
        }
        _ => panic!("catalog has no string list default for {}", path.join(".")),
    }
}

pub fn effective_default(path: &[&str]) -> Option<Value> {
    lookup(path)
        .and_then(CatalogEntry::effective_default)
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
