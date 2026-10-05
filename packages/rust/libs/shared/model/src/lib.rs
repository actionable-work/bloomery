#![allow(clippy::result_large_err)]

pub mod catalog;
pub mod config;
pub mod diagnostics;
mod domain;

pub use config::Config;
pub use diagnostics::{Diagnostic, SourceLocation, render_diagnostics, sort_diagnostics};
pub use domain::{
    Area, Context, DesignReference, EarsStatement, Evidence, Feature, MarkdownFrontmatter,
    Requirement, RequirementEntry, RequirementGroup, RequirementGroupFile,
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
