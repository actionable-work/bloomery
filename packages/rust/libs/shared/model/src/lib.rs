pub mod catalog;
pub mod clippy;
pub mod config;
pub use bloomery_schema::diagnostics;
pub use bloomery_schema::{RequirementId, is_requirement_id, parse_requirement_id};
mod domain;
pub mod health;
mod policy;
pub mod specifications;
pub mod unused_dependencies;

pub use config::Config;
pub use diagnostics::{Diagnostic, SourceLocation, render_diagnostics, sort_diagnostics};
pub use domain::{
    Area, CURRENT_REQUIREMENT_SCHEMA_VERSION, Context, DesignReference, EarsStatement, Evidence,
    Feature, MarkdownFrontmatter, Requirement, RequirementEntry, RequirementGroup,
    RequirementGroupFile, SPEC_MANAGEMENT_DISABLED_MESSAGE, TestSite, catalogue_order,
    requirement_sequence,
};

#[cfg(test)]
mod tests {
    use super::EarsStatement;

    #[test]
    fn renders_all_ears_forms() {
        let cases = [
            (
                EarsStatement::Ubiquitous {
                    system: "system".into(),
                    action: "act".into(),
                },
                "The system shall act.",
            ),
            (
                EarsStatement::Event {
                    trigger: "event".into(),
                    system: "system".into(),
                    action: "act".into(),
                },
                "When event, the system shall act.",
            ),
            (
                EarsStatement::State {
                    state: "state".into(),
                    system: "system".into(),
                    action: "act".into(),
                },
                "While state, the system shall act.",
            ),
            (
                EarsStatement::UnwantedBehavior {
                    trigger: "failure".into(),
                    system: "system".into(),
                    action: "recover".into(),
                },
                "If failure, then the system shall recover.",
            ),
            (
                EarsStatement::Optional {
                    feature: "feature".into(),
                    system: "system".into(),
                    action: "act".into(),
                },
                "Where feature, the system shall act.",
            ),
            (
                EarsStatement::Complex {
                    state: "state".into(),
                    trigger: "event".into(),
                    system: "system".into(),
                    action: "act".into(),
                },
                "While state, when event, the system shall act.",
            ),
        ];
        for (ears, expected) in cases {
            assert_eq!(ears.to_statement(), expected);
        }
    }
}
