use bloomery_scanning::{CfgExpression, RustCondition};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Caller-authorized facts, never inferred from the host or Cargo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RustCfg {
    pub target: Option<String>,
    pub features: Option<BTreeSet<String>>,
    pub flags: BTreeMap<String, bool>,
    pub values: BTreeMap<String, BTreeSet<String>>,
    pub cargo_test: Option<super::CargoTestSelection>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AttributeScope {
    Compilation,
    Harness,
}

pub(super) type Annotations = BTreeMap<bloomery_model::SourceLocation, ConditionRecord>;

impl RustCfg {
    pub fn parse_json(source: &str) -> Result<Self, String> {
        let value: serde_json::Value = serde_json::from_str(source)
            .map_err(|error| format!("invalid Rust cfg context: {error}"))?;
        if !value.is_object() {
            return Err("Rust cfg context must be a JSON object".into());
        }
        if value
            .get("cargo_test")
            .is_some_and(|target| !target.is_null() && !target.is_object())
        {
            return Err("cargo_test must be a JSON object".into());
        }
        let context: Self = serde_json::from_value(value)
            .map_err(|error| format!("invalid Rust cfg context: {error}"))?;
        context.validate()?;
        Ok(context)
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        if let Some(target) = &self.cargo_test {
            bloomery_workspace::cargo::validate_selector(&target.manifest, &target.name)
                .map_err(|error| error.message)?;
        }
        if self
            .target
            .as_ref()
            .is_some_and(|target| target.trim().is_empty())
        {
            return Err("Rust cfg target label must not be empty".into());
        }
        Ok(())
    }

    pub(super) fn evaluate(&self, expression: &CfgExpression) -> ConditionRecord {
        use CfgExpression::*;
        match expression {
            Flag(name) => self
                .flags
                .get(name)
                .filter(|_| !name.contains("::"))
                .map(|value| known(*value))
                .unwrap_or_else(|| unknown(name)),
            Value(name, value) => {
                if name.contains("::") {
                    return unknown("unsupported cfg path");
                }
                let values = if name == "feature" {
                    self.features.as_ref()
                } else {
                    self.values.get(name)
                };
                values
                    .map(|values| known(values.contains(value)))
                    .unwrap_or_else(|| unknown(&format!("{name}=\"{value}\"")))
            }
            All(items) => combine(items.iter().map(|item| self.evaluate(item)), false),
            Attributes(_) => self.gate(expression, AttributeScope::Compilation),
            List(_, _) if !supported_predicate(expression) => unknown("unsupported cfg predicate"),
            List(name, items) => match name.as_str() {
                "all" => combine(items.iter().map(|item| self.evaluate(item)), false),
                "any" => combine(items.iter().map(|item| self.evaluate(item)), true),
                "not" if items.len() == 1 => negate(self.evaluate(&items[0])),
                operator => unknown(&format!("unsupported cfg operator: {operator}")),
            },
            Unsupported(reason) => unknown(reason),
        }
    }

    fn gate(&self, expression: &CfgExpression, scope: AttributeScope) -> ConditionRecord {
        use CfgExpression::*;
        match expression {
            Attributes(items) | All(items) => {
                combine(items.iter().map(|item| self.gate(item, scope)), false)
            }
            List(name, items) if name == "cfg" && scope == AttributeScope::Compilation => {
                match items.as_slice() {
                    [predicate] => self.evaluate(predicate),
                    _ => unknown("malformed cfg attribute"),
                }
            }
            List(name, items) if name == "cfg_attr" => {
                if items.len() < 2 {
                    return unknown("malformed cfg_attr");
                }
                if !supported_predicate(&items[0]) {
                    return unknown("unsupported cfg_attr predicate");
                }
                combine(
                    [
                        negate(self.evaluate(&items[0])),
                        combine(items[1..].iter().map(|item| self.gate(item, scope)), false),
                    ]
                    .into_iter(),
                    true,
                )
            }
            Flag(name) | Value(name, _)
                if name == "cfg_attr"
                    || (name == "cfg" && scope == AttributeScope::Compilation) =>
            {
                unknown("malformed cfg attribute")
            }
            Flag(name) | List(name, _)
                if scope == AttributeScope::Compilation
                    && name.split("::").last() == Some("test") =>
            {
                unknown("conditional_test_attribute")
            }
            Flag(name) | Value(name, _) | List(name, _)
                if scope == AttributeScope::Harness && name == "ignore" =>
            {
                unknown("ignored_test_attribute")
            }
            Unsupported(reason) => unknown(reason),
            _ => known(true),
        }
    }
}

fn supported_predicate(expression: &CfgExpression) -> bool {
    use CfgExpression::*;
    match expression {
        Flag(name) | Value(name, _) => !name.contains("::"),
        List(name, items) => {
            (name == "all" || name == "any" || (name == "not" && items.len() == 1))
                && items.iter().all(supported_predicate)
        }
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionState {
    Satisfied,
    Rejected,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConditionRecord {
    pub state: ConditionState,
    pub unresolved: BTreeSet<String>,
}

fn known(value: bool) -> ConditionRecord {
    ConditionRecord {
        state: if value {
            ConditionState::Satisfied
        } else {
            ConditionState::Rejected
        },
        unresolved: BTreeSet::new(),
    }
}

pub(super) fn unknown(reason: &str) -> ConditionRecord {
    ConditionRecord {
        state: ConditionState::Unresolved,
        unresolved: BTreeSet::from([reason.into()]),
    }
}

fn negate(mut result: ConditionRecord) -> ConditionRecord {
    result.state = match result.state {
        ConditionState::Satisfied => ConditionState::Rejected,
        ConditionState::Rejected => ConditionState::Satisfied,
        ConditionState::Unresolved => ConditionState::Unresolved,
    };
    result
}

pub(super) fn combine(
    results: impl Iterator<Item = ConditionRecord>,
    any: bool,
) -> ConditionRecord {
    let mut unresolved = BTreeSet::new();
    for result in results {
        if result.state
            == if any {
                ConditionState::Satisfied
            } else {
                ConditionState::Rejected
            }
        {
            return known(any);
        }
        unresolved.extend(result.unresolved);
    }
    if unresolved.is_empty() {
        known(!any)
    } else {
        ConditionRecord {
            state: ConditionState::Unresolved,
            unresolved,
        }
    }
}

pub(super) fn annotations(context: &RustCfg, conditions: &[RustCondition]) -> Annotations {
    annotate(conditions, |condition| {
        context.evaluate(&condition.expression)
    })
}

pub(super) fn harness_annotations(
    context: Option<&RustCfg>,
    conditions: &[RustCondition],
) -> Annotations {
    let fallback = RustCfg::default();
    let context = context.unwrap_or(&fallback);
    annotate(conditions, |condition| {
        context.gate(&condition.harness, AttributeScope::Harness)
    })
}

fn annotate(
    conditions: &[RustCondition],
    evaluate: impl Fn(&RustCondition) -> ConditionRecord,
) -> Annotations {
    let mut records = BTreeMap::new();
    for condition in conditions {
        let result = evaluate(condition);
        records.insert(condition.test.clone(), result.clone());
        for reference in &condition.references {
            records.insert(reference.clone(), result.clone());
        }
    }
    records
}
