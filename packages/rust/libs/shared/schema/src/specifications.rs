//! Configuration for authored test links and explicit feature ownership.

use crate::Diagnostic;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UntiedMode {
    Off,
    #[default]
    Report,
    Deny,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecificationPolicy {
    pub enable: bool,
    pub max_tests_per_requirement: usize,
    pub max_requirements_per_test: usize,
    pub untied_tests: UntiedMode,
    pub max_untied_tests: usize,
    pub require_crate_ownership: bool,
    pub requirement_exceptions: BTreeMap<String, RequirementException>,
    pub test_exceptions: BTreeMap<String, TestException>,
    pub untied_exceptions: BTreeMap<String, UntiedException>,
    pub crates: BTreeMap<String, CrateOwnership>,
}

impl Default for SpecificationPolicy {
    fn default() -> Self {
        Self {
            enable: false,
            max_tests_per_requirement: 5,
            max_requirements_per_test: 5,
            untied_tests: UntiedMode::Report,
            max_untied_tests: 0,
            require_crate_ownership: false,
            requirement_exceptions: BTreeMap::new(),
            test_exceptions: BTreeMap::new(),
            untied_exceptions: BTreeMap::new(),
            crates: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequirementException {
    pub max_tests: usize,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TestException {
    pub path: String,
    pub name: Option<String>,
    pub max_requirements: usize,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UntiedException {
    pub reason: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct CrateOwnership {
    pub features: Vec<String>,
    pub min_requirements: Option<usize>,
    pub min_linked_tests: Option<usize>,
    pub exclude: bool,
    pub reason: Option<String>,
}

impl SpecificationPolicy {
    pub fn from_value(value: Option<&toml::Value>) -> Result<Self, Diagnostic> {
        let policy: Self = match value {
            Some(value) => value
                .clone()
                .try_into()
                .map_err(|error| invalid(format!("{error}")))?,
            None => Self::default(),
        };
        policy.validate()?;
        Ok(policy)
    }

    fn validate(&self) -> Result<(), Diagnostic> {
        self.validate_test_breadth()?;
        if self.max_tests_per_requirement == 0 {
            return Err(invalid("maxTestsPerRequirement must be positive"));
        }
        for (id, exception) in &self.requirement_exceptions {
            if !crate::is_requirement_id(id)
                || exception.max_tests == 0
                || exception.reason.trim().is_empty()
            {
                return Err(invalid(format!(
                    "invalid requirement exception '{id}': expected an ID, positive maxTests, and nonempty reason"
                )));
            }
        }
        for (pattern, exception) in &self.untied_exceptions {
            if !valid_path(pattern)
                || exception.reason.trim().is_empty()
                || exception
                    .name
                    .as_ref()
                    .is_some_and(|name| name.trim().is_empty())
            {
                return Err(invalid(format!(
                    "invalid untied exception '{pattern}': expected a relative glob, nonempty reason, and optional nonempty test name"
                )));
            }
        }
        for (manifest, ownership) in &self.crates {
            let distinct: BTreeSet<_> = ownership.features.iter().collect();
            let valid_features = !distinct.is_empty()
                && distinct.len() == ownership.features.len()
                && distinct.iter().all(|feature| {
                    let parts: Vec<_> = feature.split('/').collect();
                    parts.len() == 2
                        && parts
                            .iter()
                            .all(|part| crate::requirement_id::valid_segment(part))
                });
            let valid_exclusion = ownership
                .reason
                .as_ref()
                .is_some_and(|reason| !reason.trim().is_empty())
                && ownership.features.is_empty()
                && ownership.min_requirements.is_none()
                && ownership.min_linked_tests.is_none();
            if !valid_path(manifest)
                || manifest.split('/').next_back() != Some("Cargo.toml")
                || manifest.contains(['*', '?'])
                || ownership
                    .reason
                    .as_ref()
                    .is_some_and(|reason| reason.trim().is_empty())
                || (ownership.exclude && !valid_exclusion)
                || (!ownership.exclude && !valid_features)
            {
                return Err(invalid(format!(
                    "invalid crate ownership '{manifest}': expected a Cargo.toml path with distinct features, or an exclusion with a reason"
                )));
            }
        }
        Ok(())
    }

    fn validate_test_breadth(&self) -> Result<(), Diagnostic> {
        if self.max_requirements_per_test == 0 {
            return Err(invalid("maxRequirementsPerTest must be positive"));
        }
        for (label, exception) in &self.test_exceptions {
            if label.trim().is_empty()
                || !valid_path(&exception.path)
                || exception.max_requirements == 0
                || exception.reason.trim().is_empty()
                || exception
                    .name
                    .as_ref()
                    .is_some_and(|name| name.trim().is_empty())
            {
                return Err(invalid(format!(
                    "invalid test exception '{label}': expected a relative path glob, positive maxRequirements, nonempty reason, and optional nonempty name"
                )));
            }
        }
        Ok(())
    }
}

fn valid_path(value: &str) -> bool {
    !value.trim().is_empty()
        && !value.contains(['\\', ':', '\0'])
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn invalid(message: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::new(
        "ConfigurationError",
        format!("checks.specifications: {message}"),
    )
}
