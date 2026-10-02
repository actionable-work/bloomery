use crate::config::Config;
use crate::diagnostics::SourceLocation;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct MarkdownFrontmatter {
    pub id: String,
    pub name: String,
    pub tagline: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequirementGroupFile {
    pub group: String,
    pub requirements: Vec<RequirementEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequirementEntry {
    pub id: String,
    pub title: String,
    pub manual: bool,
    pub design: Option<String>,
    pub ears: EarsStatement,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
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

#[derive(Debug, Clone)]
pub struct DesignReference {
    pub path: PathBuf,
    pub display: String,
    pub anchor: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Requirement {
    pub entry: RequirementEntry,
    pub group: String,
    pub path: PathBuf,
    pub line: usize,
    pub design: DesignReference,
}

#[derive(Debug, Clone)]
pub struct RequirementGroup {
    pub group: String,
    pub path: PathBuf,
    pub requirements: Vec<Requirement>,
}

#[derive(Debug, Clone)]
pub struct Feature {
    pub id: String,
    pub path: PathBuf,
    pub frontmatter: MarkdownFrontmatter,
    pub groups: Vec<RequirementGroup>,
}

#[derive(Debug, Clone)]
pub struct Service {
    pub id: String,
    pub path: PathBuf,
    pub frontmatter: MarkdownFrontmatter,
    pub features: Vec<Feature>,
}

#[derive(Debug, Clone)]
pub struct Evidence {
    pub id: String,
    pub location: SourceLocation,
    pub scanner: &'static str,
}

#[derive(Debug, Clone)]
pub struct Context {
    pub root: PathBuf,
    pub config: Config,
    pub services: Vec<Service>,
}

impl Context {
    pub fn requirements(&self) -> impl Iterator<Item = (&Service, &Feature, &Requirement)> {
        self.services.iter().flat_map(|service| {
            service.features.iter().flat_map(move |feature| {
                feature.groups.iter().flat_map(move |group| {
                    group
                        .requirements
                        .iter()
                        .map(move |requirement| (service, feature, requirement))
                })
            })
        })
    }
}
