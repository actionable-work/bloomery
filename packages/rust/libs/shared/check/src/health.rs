//! Deterministic score arithmetic and policy evaluation for code health.

use bloomery_model::health::HealthConfig;
use bloomery_scanning::HealthCrateMetrics;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Penalties {
    pub complexity: f64,
    pub size: f64,
    pub nesting: f64,
    pub duplication: f64,
    pub crate_size: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrateScore {
    pub duplication_fraction: f64,
    pub penalties: Penalties,
    pub score_hundredths: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthGate {
    pub code: String,
    pub message: String,
    pub subject: Option<String>,
    pub path: Option<String>,
    pub line: Option<usize>,
}

pub fn score_crate(metrics: &HealthCrateMetrics, policy: &HealthConfig) -> CrateScore {
    let callable_count = metrics.callables.len();
    let mean = |values: Vec<f64>| {
        if callable_count == 0 {
            0.0
        } else {
            values.into_iter().sum::<f64>() / callable_count as f64
        }
    };
    let complexity_rate = mean(
        metrics
            .callables
            .iter()
            .map(|callable| excess(callable.cyclomatic, policy.cyclomatic_limit))
            .collect(),
    );
    let size_rate = mean(
        metrics
            .callables
            .iter()
            .map(|callable| excess(callable.body_tokens.len(), policy.function_tokens_limit))
            .collect(),
    );
    let nesting_rate = mean(
        metrics
            .callables
            .iter()
            .map(|callable| excess(callable.nesting, policy.nesting_limit))
            .collect(),
    );
    let duplication_fraction = if metrics.callable_tokens == 0 {
        0.0
    } else {
        metrics.duplicate_tokens as f64 / metrics.callable_tokens as f64
    };
    let duplication_rate = ((duplication_fraction - policy.duplication_free_ratio)
        / (1.0 - policy.duplication_free_ratio))
        .clamp(0.0, 1.0);
    let crate_size_rate = excess(metrics.sloc, policy.crate_sloc_limit);
    let weight_sum = policy.weights.total() as f64;
    let weighted_penalty = |weight: u16, rate: f64| 100.0 * (f64::from(weight) / weight_sum) * rate;
    let complexity = weighted_penalty(policy.weights.complexity, complexity_rate);
    let size = weighted_penalty(policy.weights.size, size_rate);
    let nesting = weighted_penalty(policy.weights.nesting, nesting_rate);
    let duplication = weighted_penalty(policy.weights.duplication, duplication_rate);
    let crate_size = weighted_penalty(policy.weights.crate_size, crate_size_rate);
    let penalties = Penalties {
        complexity,
        size,
        nesting,
        duplication,
        crate_size,
    };
    let score_hundredths = (metrics.sloc > 0).then(|| {
        let score =
            (100.0 - complexity - size - nesting - duplication - crate_size).clamp(0.0, 100.0);
        (score * 100.0 + 0.5).floor() as u16
    });
    CrateScore {
        duplication_fraction,
        penalties,
        score_hundredths,
    }
}

pub fn workspace_score(crates: &[HealthCrateMetrics], scores: &[CrateScore]) -> Option<u16> {
    let total_sloc = crates
        .iter()
        .map(|crate_metrics| crate_metrics.sloc)
        .sum::<usize>();
    if total_sloc == 0 {
        return None;
    }
    let weighted = crates
        .iter()
        .zip(scores)
        .filter_map(|(crate_metrics, score)| {
            score
                .score_hundredths
                .map(|score| score as u128 * crate_metrics.sloc as u128)
        })
        .sum::<u128>();
    let applicable_sloc = crates
        .iter()
        .zip(scores)
        .filter(|(_, score)| score.score_hundredths.is_some())
        .map(|(crate_metrics, _)| crate_metrics.sloc as u128)
        .sum::<u128>();
    if applicable_sloc == 0 {
        return None;
    }
    let numerator = weighted;
    let denominator = applicable_sloc;
    Some(((numerator + denominator / 2) / denominator).min(10_000) as u16)
}

pub fn policy_gates(
    crates: &[HealthCrateMetrics],
    scores: &[CrateScore],
    workspace: Option<u16>,
    policy: &HealthConfig,
) -> Vec<HealthGate> {
    let mut failures = Vec::new();
    for (metrics, score) in crates.iter().zip(scores) {
        let hard_limits = policy.hard_limits_for_crate(&metrics.manifest_path);
        if let Some(score_value) = score.score_hundredths
            && score_value < policy.min_crate_score
        {
            failures.push(HealthGate {
                code: "HealthCrateScoreBelowFloor".to_owned(),
                message: format!(
                    "crate score {:.2} is below configured minimum {:.2}",
                    score_value as f64 / 100.0,
                    policy.min_crate_score as f64 / 100.0,
                ),
                subject: Some(metrics.manifest_path.clone()),
                path: None,
                line: None,
            });
        }
        if metrics.sloc > hard_limits.max_crate_sloc {
            failures.push(HealthGate {
                code: "HealthCrateSizeLimitExceeded".to_owned(),
                message: format!(
                    "production SLOC {} exceeds configured maxCrateSloc {}",
                    metrics.sloc, hard_limits.max_crate_sloc,
                ),
                subject: Some(metrics.manifest_path.clone()),
                path: None,
                line: None,
            });
        }
        for callable in &metrics.callables {
            let limits = [
                (
                    "maxCyclomatic",
                    callable.cyclomatic,
                    hard_limits.max_cyclomatic,
                ),
                (
                    "maxFunctionTokens",
                    callable.body_tokens.len(),
                    hard_limits.max_function_tokens,
                ),
                ("maxNesting", callable.nesting, hard_limits.max_nesting),
            ];
            for (name, measured, limit) in limits {
                if measured > limit {
                    failures.push(HealthGate {
                        code: "HealthCallableLimitExceeded".to_owned(),
                        message: format!(
                            "{name} value {measured} exceeds configured limit {limit}"
                        ),
                        subject: Some(format!("{}::{}", metrics.manifest_path, callable.identity)),
                        path: Some(callable.path.clone()),
                        line: Some(callable.start_line),
                    });
                }
            }
        }
    }
    if let Some(workspace_score) = workspace
        && workspace_score < policy.min_workspace_score
    {
        failures.push(HealthGate {
            code: "HealthWorkspaceScoreBelowFloor".to_owned(),
            message: format!(
                "workspace score {:.2} is below configured minimum {:.2}",
                workspace_score as f64 / 100.0,
                policy.min_workspace_score as f64 / 100.0,
            ),
            subject: None,
            path: None,
            line: None,
        });
    }
    failures
}

fn excess(value: usize, limit: usize) -> f64 {
    ((value as f64 - limit as f64) / limit as f64).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::{policy_gates, score_crate, workspace_score};
    use bloomery_model::health::HealthConfig;
    use bloomery_scanning::HealthToken;
    use bloomery_scanning::{HealthCallable, HealthCrateMetrics};

    fn crate_metrics(manifest: &str, sloc: usize, complexities: &[usize]) -> HealthCrateMetrics {
        let callables = complexities
            .iter()
            .enumerate()
            .map(|(index, complexity)| HealthCallable {
                identity: format!("f{index}"),
                path: "src/lib.rs".to_owned(),
                start_line: index + 1,
                start_column: 0,
                end_line: index + 1,
                end_column: 10,
                cyclomatic: *complexity,
                nesting: 0,
                body_tokens: Vec::new(),
                duplicate_token_count: 0,
            })
            .collect();
        HealthCrateMetrics {
            name: manifest.to_owned(),
            manifest_path: manifest.to_owned(),
            sources: vec!["src/lib.rs".to_owned()],
            opaque_sources: Vec::new(),
            sloc,
            callables,
            duplicate_tokens: 0,
            callable_tokens: 0,
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-004"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-008"))]
    fn score_uses_mean_excess_and_hundredths() {
        let policy = HealthConfig::default();
        let metrics = crate_metrics("a/Cargo.toml", 10, &[10, 11]);
        let score = score_crate(&metrics, &policy);
        assert_eq!(score.penalties.complexity, 1.25);
        assert_eq!(score.score_hundredths, Some(9_875));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-005"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-006"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-007"))]
    fn workspace_uses_crate_slok_weight_and_ignores_empty_crates() {
        let policy = HealthConfig::default();
        let crates = vec![
            crate_metrics("a/Cargo.toml", 90, &[10]),
            crate_metrics("b/Cargo.toml", 10, &[20]),
            crate_metrics("empty/Cargo.toml", 0, &[100]),
        ];
        let scores = crates
            .iter()
            .map(|item| score_crate(item, &policy))
            .collect::<Vec<_>>();
        assert_eq!(scores[2].score_hundredths, None);
        assert_eq!(workspace_score(&crates, &scores), Some(9_750));
        assert_eq!(workspace_score(&[], &[]), None);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-009"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-010"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-011"))]
    fn score_floors_and_callable_hard_limits_are_independent_gates() {
        let mut policy = HealthConfig::default();
        policy.min_crate_score = 9_999;
        policy.min_workspace_score = 9_999;
        policy.cyclomatic_limit = 1;
        policy.max_cyclomatic = 4;
        policy.function_tokens_limit = 1;
        policy.max_function_tokens = 1;
        policy.nesting_limit = 1;
        policy.max_nesting = 2;
        let mut metrics = crate_metrics("demo/Cargo.toml", 10, &[5]);
        metrics.callables[0].nesting = 3;
        metrics.callables[0].body_tokens = vec![
            HealthToken {
                kind: "identifier".to_owned(),
                text: "value".to_owned(),
                start_line: 1,
                start_column: 0,
                end_line: 1,
                end_column: 5,
            };
            2
        ];
        metrics.callable_tokens = 2;
        let score = score_crate(&metrics, &policy);
        assert_eq!(score.penalties.size, 20.0);
        let gates = policy_gates(
            &[metrics],
            &[score],
            Some(score.score_hundredths.unwrap()),
            &policy,
        );
        assert!(
            gates
                .iter()
                .any(|gate| gate.code == "HealthCrateScoreBelowFloor")
        );
        assert!(
            gates
                .iter()
                .any(|gate| gate.code == "HealthWorkspaceScoreBelowFloor")
        );
        assert_eq!(
            gates
                .iter()
                .filter(|gate| gate.code == "HealthCallableLimitExceeded")
                .count(),
            3,
            "cyclomatic, token count, and nesting have separate hard gates"
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-004"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-040"))]
    fn configured_weights_control_the_crate_size_deduction() {
        let mut policy = HealthConfig::default();
        policy.weights = bloomery_model::health::HealthWeights {
            complexity: 4_000,
            size: 0,
            nesting: 0,
            duplication: 0,
            crate_size: 6_000,
        };
        let metrics = crate_metrics("large/Cargo.toml", 2_250, &[15]);
        let score = score_crate(&metrics, &policy);
        assert_eq!(score.penalties.crate_size, 30.0);
        assert_eq!(score.penalties.complexity, 20.0);
        assert_eq!(score.score_hundredths, Some(5_000));
        let small = crate_metrics("small/Cargo.toml", 750, &[]);
        let small_score = score_crate(&small, &policy);
        assert_eq!(
            workspace_score(&[metrics, small], &[score, small_score]),
            Some(6_250)
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-041"))]
    fn crate_size_hard_limit_is_independent_of_score_floors() {
        let mut policy = HealthConfig::default();
        policy.min_crate_score = 0;
        policy.min_workspace_score = 0;
        policy.max_crate_sloc = 2_000;
        let metrics = crate_metrics("large/Cargo.toml", 2_001, &[]);
        let score = score_crate(&metrics, &policy);
        let gates = policy_gates(&[metrics], &[score], Some(10_000), &policy);
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0].code, "HealthCrateSizeLimitExceeded");
        assert!(gates[0].message.contains("2001"));
        assert!(gates[0].message.contains("2000"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-044"))]
    fn crate_exceptions_change_hard_gates_for_only_the_matching_manifest() {
        use std::collections::BTreeMap;

        use bloomery_model::health::HealthLimitOverrides;

        let mut policy = HealthConfig {
            min_crate_score: 0,
            min_workspace_score: 0,
            ..HealthConfig::default()
        };
        policy.crate_exceptions = BTreeMap::from([(
            "overridden/Cargo.toml".to_owned(),
            HealthLimitOverrides {
                max_cyclomatic: Some(50),
                max_crate_sloc: Some(6_000),
                ..HealthLimitOverrides::default()
            },
        )]);
        let overridden = crate_metrics("overridden/Cargo.toml", 5_001, &[41]);
        let other = crate_metrics("other/Cargo.toml", 5_001, &[41]);
        let scores = [
            score_crate(&overridden, &policy),
            score_crate(&other, &policy),
        ];
        let gates = policy_gates(&[overridden, other], &scores, Some(10_000), &policy);

        assert_eq!(gates.len(), 2);
        assert!(
            gates
                .iter()
                .all(|gate| gate.subject.as_deref() == Some("other/Cargo.toml")
                    || gate.subject.as_deref() == Some("other/Cargo.toml::f0"))
        );
        assert!(
            gates
                .iter()
                .any(|gate| gate.code == "HealthCrateSizeLimitExceeded")
        );
        assert!(
            gates
                .iter()
                .any(|gate| gate.code == "HealthCallableLimitExceeded")
        );
    }
}
