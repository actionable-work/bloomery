use crate::config::Config;
use crate::diagnostics::SourceLocation;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct MarkdownFrontmatter {
    pub id: String,
    pub name: String,
    pub tagline: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RequirementGroupFile {
    pub schema_version: Option<toml::Spanned<i64>>,
    pub group: String,
    pub requirements: Vec<RequirementEntry>,
}

pub const CURRENT_REQUIREMENT_SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RequirementEntry {
    pub id: String,
    pub title: String,
    pub manual: bool,
    pub design: Option<String>,
    pub ears: EarsStatement,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum EarsStatement {
    Ubiquitous {
        system: String,
        action: String,
    },
    Event {
        trigger: String,
        system: String,
        action: String,
    },
    State {
        state: String,
        system: String,
        action: String,
    },
    UnwantedBehavior {
        trigger: String,
        system: String,
        action: String,
    },
    Optional {
        feature: String,
        system: String,
        action: String,
    },
    Complex {
        state: String,
        trigger: String,
        system: String,
        action: String,
    },
}

impl EarsStatement {
    pub fn to_statement(&self) -> String {
        match self {
            Self::Ubiquitous { system, action } => format!("The {system} shall {action}."),
            Self::Event {
                trigger,
                system,
                action,
            } => format!("When {trigger}, the {system} shall {action}."),
            Self::State {
                state,
                system,
                action,
            } => format!("While {state}, the {system} shall {action}."),
            Self::UnwantedBehavior {
                trigger,
                system,
                action,
            } => format!("If {trigger}, then the {system} shall {action}."),
            Self::Optional {
                feature,
                system,
                action,
            } => format!("Where {feature}, the {system} shall {action}."),
            Self::Complex {
                state,
                trigger,
                system,
                action,
            } => format!("While {state}, when {trigger}, the {system} shall {action}."),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignReference {
    pub path: PathBuf,
    pub display: String,
    pub anchor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    pub entry: RequirementEntry,
    pub group: String,
    pub path: PathBuf,
    pub line: usize,
    pub design: DesignReference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementGroup {
    pub group: String,
    pub path: PathBuf,
    pub requirements: Vec<Requirement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feature {
    pub id: String,
    pub path: PathBuf,
    pub frontmatter: MarkdownFrontmatter,
    pub groups: Vec<RequirementGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Area {
    pub id: String,
    pub path: PathBuf,
    pub frontmatter: MarkdownFrontmatter,
    pub features: Vec<Feature>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    pub id: String,
    pub location: SourceLocation,
    pub scanner: &'static str,
}

/// A statically discovered test declaration with the requirement references it
/// carries. An empty `references` vector marks an untied graduation candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestSite {
    pub scanner: &'static str,
    pub name: Option<String>,
    pub location: SourceLocation,
    pub references: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Context {
    pub root: PathBuf,
    pub config: Config,
    /// Specification enablement resolved against the analyzed tree.
    pub specs_enabled: bool,
    pub areas: Vec<Area>,
}

impl Context {
    /// Whether specification management is active for this workspace.
    pub fn spec_management_enabled(&self) -> bool {
        self.specs_enabled
    }

    pub fn requirements(&self) -> impl Iterator<Item = (&Area, &Feature, &Requirement)> {
        self.areas.iter().flat_map(|area| {
            area.features.iter().flat_map(move |feature| {
                feature.groups.iter().flat_map(move |group| {
                    group
                        .requirements
                        .iter()
                        .map(move |requirement| (area, feature, requirement))
                })
            })
        })
    }
}
