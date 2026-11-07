use bloomery_model::{Diagnostic, SourceLocation};
use serde::Serialize;

/// Completion of a static stage, independent of its findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StageState {
    NotAttempted,
    Complete,
    Partial,
    Disabled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Completeness {
    pub configuration: StageState,
    pub tree: StageState,
    pub specifications: StageState,
    pub discovery: StageState,
    pub relations: StageState,
    pub rust_cfg: StageState,
    pub rust_harness: StageState,
    pub rust_target: StageState,
    pub rust_modules: StageState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InputIdentities {
    pub tree: Option<String>,
    pub configuration: String,
    pub scope: String,
    pub specifications: Option<String>,
    pub parser_contract: String,
    pub specification_schema: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Scope {
    pub kind: String,
    pub target_resolution: String,
    pub feature_resolution: String,
    pub conditional_compilation: String,
    pub rust_cfg: Option<super::RustCfg>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScannerState {
    NotAttempted,
    Disabled,
    NoMatchingFiles,
    Complete,
    /// The aggregate scanner failed and returned no facts for any scanner.
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScannerStatus {
    pub scanner: String,
    pub state: ScannerState,
}

/// Available one-based Unicode scalar positions, with exclusive ends.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Location {
    pub path: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub end_line: Option<usize>,
    pub end_column: Option<usize>,
}

impl From<&SourceLocation> for Location {
    fn from(value: &SourceLocation) -> Self {
        Self {
            path: value
                .path
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/"),
            line: value.line,
            column: value.column,
            end_line: value.end_line,
            end_column: value.end_column,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Finding {
    pub location: Option<Location>,
    pub code: String,
    pub message: String,
    pub notes: Vec<String>,
}

impl From<Diagnostic> for Finding {
    fn from(value: Diagnostic) -> Self {
        Self {
            location: value.location.as_deref().map(Location::from),
            code: value.code,
            message: value.message,
            notes: value.notes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Coverage {
    Manual,
    Bound,
    Missing,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RequirementRecord {
    pub id: String,
    pub area: String,
    pub feature: String,
    pub group: String,
    pub title: String,
    pub manual: bool,
    pub statement: String,
    pub location: Location,
    pub design_path: String,
    pub design_anchor: Option<String>,
    pub coverage: Coverage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TestRecord {
    pub scanner: String,
    pub name: Option<String>,
    pub location: Location,
    pub references: Vec<String>,
    pub rust_cfg: Option<super::ConditionRecord>,
    pub rust_harness: Option<super::ConditionRecord>,
    pub rust_target: Option<super::ConditionRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingResolution {
    Declared,
    Manual,
    Unresolved,
    SpecificationsUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Binding {
    pub requirement_id: String,
    pub scanner: String,
    pub location: Location,
    pub resolution: BindingResolution,
    pub rust_cfg: Option<super::ConditionRecord>,
    pub rust_harness: Option<super::ConditionRecord>,
    pub rust_target: Option<super::ConditionRecord>,
}

/// Versioned static facts. Never an execution result or approval decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StaticReport {
    pub version: u32,
    /// SHA-256 of the canonical payload with this field omitted.
    pub identity: String,
    pub inputs: InputIdentities,
    pub configuration: serde_json::Value,
    pub scope: Scope,
    pub cargo_test: Option<super::CargoTestRecord>,
    pub completeness: Completeness,
    pub scanners: Vec<ScannerStatus>,
    pub requirements: Vec<RequirementRecord>,
    pub tests: Vec<TestRecord>,
    pub bindings: Vec<Binding>,
    pub diagnostics: Vec<Finding>,
}
