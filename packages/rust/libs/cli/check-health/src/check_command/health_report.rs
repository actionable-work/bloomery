use super::model::{FailureDraft, FailureLocation};
use bloomery_check::{AnalysisSession, HealthSourceCrate};
use bloomery_model::{Diagnostic, health::HealthConfig};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub struct HealthOutcome {
    pub report: Value,
    pub failures: Vec<FailureDraft>,
}
pub fn collect_health_report(
    root: &Path,
    source_projection: &Result<Vec<bloomery_check::HealthSourceCrate>, String>,
    session: &AnalysisSession,
    no_write: bool,
) -> Result<HealthOutcome, Vec<Diagnostic>> {
    let loaded =
        bloomery_model::config::load_with_raw(root).map_err(|diagnostic| vec![diagnostic])?;
    let config =
        HealthConfig::from_raw(loaded.raw.as_ref()).map_err(|diagnostic| vec![diagnostic])?;
    let sources = source_projection
        .clone()
        .map_err(|error| vec![Diagnostic::new("HealthDiscoveryError", error)])?;
    let mut crates =
        bloomery_check::analyze_health(root, &sources, config.duplicate_min_tokens, session)?;
    if let Err(message) = validate_health_metrics_overlap(root, &config, &crates) {
        return Err(vec![Diagnostic::new("ConfigurationError", message)]);
    }
    crates.sort_by(|left, right| left.manifest_path.cmp(&right.manifest_path));
    let scores = crates
        .iter()
        .map(|crate_metrics| bloomery_check::health::score_crate(crate_metrics, &config))
        .collect::<Vec<_>>();
    let workspace = bloomery_check::health::workspace_score(&crates, &scores);
    let mut failures = bloomery_check::health::policy_gates(&crates, &scores, workspace, &config)
        .into_iter()
        .map(|gate| {
            let location = gate.path.map(|path| FailureLocation {
                path,
                line: gate.line,
                column: None,
            });
            FailureDraft {
                check: "static:code-health".to_owned(),
                code: gate.code,
                subject: gate.subject,
                location,
                message: gate.message,
                notes: Vec::new(),
                log: None,
                nix_log: None,
                focus_tail: false,
                occurrence: 0,
            }
        })
        .collect::<Vec<_>>();
    let raw_json = loaded
        .raw
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|error| vec![Diagnostic::new("HealthInputError", error.to_string())])?;
    let policy = health_policy_value(&config, &raw_json);
    let input_digest = health_input_digest(root, &crates, &sources, &policy, session)
        .map_err(|error| vec![Diagnostic::new("HealthInputError", error)])?;
    let snapshot = health_snapshot(&crates, &scores, workspace, policy, input_digest);
    let mut report = snapshot.clone();
    failures.extend(compare_health_baseline(
        HealthBaselineInputs {
            root,
            config: &config,
            crates: &crates,
            scores: &scores,
            workspace,
            no_write,
            snapshot: &snapshot,
        },
        &mut report,
    ));
    failures.sort_by(|left, right| {
        (
            &left.code,
            &left.subject,
            left.location
                .as_ref()
                .map(|location| (&location.path, location.line)),
            &left.message,
        )
            .cmp(&(
                &right.code,
                &right.subject,
                right
                    .location
                    .as_ref()
                    .map(|location| (&location.path, location.line)),
                &right.message,
            ))
    });
    for (index, failure) in failures.iter_mut().enumerate() {
        failure.occurrence = index;
    }
    Ok(HealthOutcome { report, failures })
}

struct HealthBaselineInputs<'a> {
    root: &'a Path,
    config: &'a HealthConfig,
    crates: &'a [bloomery_check::HealthCrateMetrics],
    scores: &'a [bloomery_check::health::CrateScore],
    workspace: Option<u16>,
    no_write: bool,
    snapshot: &'a Value,
}

fn compare_health_baseline(
    inputs: HealthBaselineInputs<'_>,
    report: &mut Value,
) -> Vec<FailureDraft> {
    let HealthBaselineInputs {
        root,
        config,
        crates,
        scores,
        workspace,
        no_write,
        snapshot,
    } = inputs;
    let (status, baseline, mut failures) = read_health_baseline(root, config, workspace, no_write);
    report["baseline_status"] = json!(status);
    report["baseline_workspace_score_hundredths"] = Value::Null;
    report["workspace_delta_hundredths"] = Value::Null;
    report["added_crates"] = json!([]);
    report["removed_crates"] = json!([]);
    let Some(baseline) = baseline else {
        return failures;
    };
    match validate_health_baseline(&baseline, snapshot) {
        Ok(baseline) => compare_compatible_baseline(CompatibleBaselineInputs {
            config,
            crates,
            scores,
            workspace,
            baseline_workspace: baseline.workspace_score_hundredths,
            baseline_crates: &baseline.crates,
            report,
            failures: &mut failures,
        }),
        Err(message) => {
            report["baseline_status"] = json!("incompatible");
            failures.push(health_failure(
                "HealthBaselineIncompatible",
                message,
                Some(config.baseline.clone()),
                None,
            ));
        }
    }
    failures
}

fn read_health_baseline(
    root: &Path,
    config: &HealthConfig,
    workspace: Option<u16>,
    no_write: bool,
) -> (&'static str, Option<Value>, Vec<FailureDraft>) {
    let result = std::fs::read(root.join(&config.baseline));
    match result {
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(value) => ("loaded", Some(value), Vec::new()),
            Err(error) => (
                "malformed",
                None,
                vec![health_failure(
                    "HealthBaselineInvalid",
                    format!("code health baseline is malformed: {error}"),
                    Some(config.baseline.clone()),
                    None,
                )],
            ),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let applicable = workspace.is_some();
            let failures = (applicable && no_write)
                .then(|| {
                    health_failure(
                        "HealthBaselineMissing",
                        format!(
                            "code health baseline '{}' does not exist; rerun `bloomery check` in write mode or publish a retained measurement with `bloomery check health update`",
                            config.baseline
                        ),
                        Some(config.baseline.clone()),
                        None,
                    )
                })
                .into_iter()
                .collect();
            let status = if applicable && !no_write {
                "initialized"
            } else {
                "missing"
            };
            (status, None, failures)
        }
        Err(error) => (
            "unreadable",
            None,
            vec![health_failure(
                "HealthBaselineInvalid",
                format!("unable to read code health baseline: {error}"),
                Some(config.baseline.clone()),
                None,
            )],
        ),
    }
}

struct CompatibleBaselineInputs<'a> {
    config: &'a HealthConfig,
    crates: &'a [bloomery_check::HealthCrateMetrics],
    scores: &'a [bloomery_check::health::CrateScore],
    workspace: Option<u16>,
    baseline_workspace: Option<u16>,
    baseline_crates: &'a BTreeMap<String, Option<u16>>,
    report: &'a mut Value,
    failures: &'a mut Vec<FailureDraft>,
}

fn compare_compatible_baseline(inputs: CompatibleBaselineInputs<'_>) {
    let CompatibleBaselineInputs {
        config,
        crates,
        scores,
        workspace,
        baseline_workspace,
        baseline_crates,
        report,
        failures,
    } = inputs;
    report["baseline_status"] = json!("compatible");
    report["baseline_workspace_score_hundredths"] =
        baseline_workspace.map_or(Value::Null, |score| json!(score));
    compare_workspace_score(config, workspace, baseline_workspace, report, failures);
    compare_crate_membership(crates, baseline_crates, report);
    report["crate_comparisons"] = json!(compare_crate_scores(
        config,
        crates,
        scores,
        baseline_crates,
        failures,
    ));
}

fn compare_workspace_score(
    config: &HealthConfig,
    current: Option<u16>,
    previous: Option<u16>,
    report: &mut Value,
    failures: &mut Vec<FailureDraft>,
) {
    if let (Some(current), Some(previous)) = (current, previous) {
        let delta = i32::from(current) - i32::from(previous);
        report["workspace_delta_hundredths"] = json!(delta);
        if delta < -i32::from(config.max_regression) {
            failures.push(health_failure(
                "HealthWorkspaceRegression",
                regression_message("workspace", delta, config.max_regression),
                None,
                None,
            ));
        }
    }
}

fn compare_crate_membership(
    crates: &[bloomery_check::HealthCrateMetrics],
    baseline_crates: &BTreeMap<String, Option<u16>>,
    report: &mut Value,
) {
    let current_ids = crates
        .iter()
        .map(|crate_metrics| crate_metrics.manifest_path.as_str())
        .collect::<BTreeSet<_>>();
    let previous_ids = baseline_crates
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    report["added_crates"] = json!(current_ids.difference(&previous_ids).collect::<Vec<_>>());
    report["removed_crates"] = json!(previous_ids.difference(&current_ids).collect::<Vec<_>>());
}

fn compare_crate_scores(
    config: &HealthConfig,
    crates: &[bloomery_check::HealthCrateMetrics],
    scores: &[bloomery_check::health::CrateScore],
    baseline_crates: &BTreeMap<String, Option<u16>>,
    failures: &mut Vec<FailureDraft>,
) -> Vec<Value> {
    crates
        .iter()
        .zip(scores)
        .map(|(crate_metrics, score)| {
            let previous = baseline_crates
                .get(&crate_metrics.manifest_path)
                .copied()
                .flatten();
            let delta = score
                .score_hundredths
                .zip(previous)
                .map(|(now, before)| i32::from(now) - i32::from(before));
            if let Some(delta) = delta
                && delta < -i32::from(config.max_regression)
            {
                failures.push(health_failure(
                    "HealthCrateRegression",
                    regression_message("crate", delta, config.max_regression),
                    Some(crate_metrics.manifest_path.clone()),
                    None,
                ));
            }
            json!({
                "manifest_path": crate_metrics.manifest_path,
                "baseline_score_hundredths": previous,
                "score_hundredths": score.score_hundredths,
                "delta_hundredths": delta,
            })
        })
        .collect()
}

fn regression_message(scope: &str, delta: i32, allowance: u16) -> String {
    format!(
        "{scope} score regressed by {:.2} points, exceeding the {:.2} point allowance",
        -delta as f64 / 100.0,
        allowance as f64 / 100.0,
    )
}

fn health_failure(
    code: &str,
    message: String,
    subject: Option<String>,
    location: Option<FailureLocation>,
) -> FailureDraft {
    FailureDraft {
        check: "static:code-health".to_owned(),
        code: code.to_owned(),
        subject,
        location,
        message,
        notes: Vec::new(),
        log: None,
        nix_log: None,
        focus_tail: false,
        occurrence: 0,
    }
}

pub fn validate_health_source_overlap(
    root: &Path,
    config: &HealthConfig,
    raw: &Option<Value>,
    sources: &[bloomery_check::HealthSourceCrate],
) -> Result<(), String> {
    let root = std::fs::canonicalize(root)
        .map_err(|error| format!("unable to resolve repository root: {error}"))?;
    let baseline = Path::new(&config.baseline);
    let candidate = root.join(baseline);
    let selected_manifest = raw
        .as_ref()
        .and_then(|value| value.get("build"))
        .and_then(Value::as_object)
        .and_then(|build| build.get("cargoToml"))
        .and_then(Value::as_str)
        .unwrap_or("Cargo.toml");
    let mut protected = BTreeSet::from([
        root.join(".bloomery/config.toml"),
        root.join(".bloomery/specs"),
        root.join("Cargo.lock"),
        root.join("flake.lock"),
        root.join(selected_manifest),
    ]);
    for crate_source in sources {
        protected.insert(root.join(&crate_source.manifest_path));
        let crate_directory = root
            .join(&crate_source.manifest_path)
            .parent()
            .unwrap_or(&root)
            .to_path_buf();
        for target in &crate_source.targets {
            protected.insert(crate_directory.join(&target.path));
        }
    }
    if protected
        .iter()
        .any(|path| candidate.starts_with(path) || path.starts_with(&candidate))
    {
        return Err("checks.codeHealth.baseline overlaps a manifest, source entrypoint, configuration, lockfile, or specification".to_owned());
    }
    Ok(())
}

fn validate_health_metrics_overlap(
    root: &Path,
    config: &HealthConfig,
    crates: &[bloomery_check::HealthCrateMetrics],
) -> Result<(), String> {
    let root = std::fs::canonicalize(root)
        .map_err(|error| format!("unable to resolve repository root: {error}"))?;
    let candidate = root.join(&config.baseline);
    if crates.iter().any(|crate_metrics| {
        crate_metrics.sources.iter().any(|source| {
            candidate == root.join(source) || root.join(source).starts_with(&candidate)
        })
    }) {
        return Err("checks.codeHealth.baseline overlaps a resolved Rust source file".to_owned());
    }
    Ok(())
}

pub fn health_policy_value(config: &HealthConfig, raw: &Option<Value>) -> Value {
    let build = raw
        .as_ref()
        .and_then(|value| value.get("build"))
        .and_then(Value::as_object);
    let selected_manifest = build
        .and_then(|build| build.get("cargoToml"))
        .and_then(Value::as_str)
        .unwrap_or("Cargo.toml");
    let members = build
        .and_then(|build| build.get("members"))
        .and_then(Value::as_array)
        .map(|members| {
            let mut members = members.iter().filter_map(Value::as_str).collect::<Vec<_>>();
            members.sort();
            json!(members)
        })
        .unwrap_or(Value::Null);
    let crate_exceptions = config
        .crate_exceptions
        .keys()
        .map(|manifest| {
            let limits = config.hard_limits_for_crate(manifest);
            (
                manifest.clone(),
                json!({
                    "maxCyclomatic": limits.max_cyclomatic,
                    "maxFunctionTokens": limits.max_function_tokens,
                    "maxNesting": limits.max_nesting,
                    "maxCrateSloc": limits.max_crate_sloc,
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    json!({
        "scoring": {
            "cyclomaticLimit": config.cyclomatic_limit,
            "functionTokensLimit": config.function_tokens_limit,
            "nestingLimit": config.nesting_limit,
            "crateSlocLimit": config.crate_sloc_limit,
            "duplicationFreeRatio": config.duplication_free_ratio,
            "duplicateMinTokens": config.duplicate_min_tokens,
            "weights": {
                "complexity": f64::from(config.weights.complexity) / 100.0,
                "size": f64::from(config.weights.size) / 100.0,
                "nesting": f64::from(config.weights.nesting) / 100.0,
                "duplication": f64::from(config.weights.duplication) / 100.0,
                "crateSize": f64::from(config.weights.crate_size) / 100.0,
            },
        },
        "selection": {
            "manifest": selected_manifest,
            "members": members,
        },
        "effective": {
            "enable": config.enabled,
            "baseline": config.baseline,
            "minCrateScore": config.min_crate_score,
            "minWorkspaceScore": config.min_workspace_score,
            "maxRegression": config.max_regression,
            "maxCyclomatic": config.max_cyclomatic,
            "maxFunctionTokens": config.max_function_tokens,
            "maxNesting": config.max_nesting,
            "maxCrateSloc": config.max_crate_sloc,
            "crateExceptions": crate_exceptions,
        },
    })
}

fn health_input_digest(
    root: &Path,
    crates: &[bloomery_check::HealthCrateMetrics],
    sources: &[bloomery_check::HealthSourceCrate],
    policy: &Value,
    session: &AnalysisSession,
) -> Result<String, String> {
    let mut hasher = Sha256::new();
    let mut inputs = BTreeSet::new();
    let selected_manifest = policy
        .pointer("/selection/manifest")
        .and_then(Value::as_str)
        .unwrap_or("Cargo.toml");
    inputs.insert(selected_manifest.to_owned());
    inputs.insert(".bloomery/config.toml".to_owned());
    for crate_source in sources {
        inputs.insert(crate_source.manifest_path.clone());
    }
    let source_paths = crates
        .iter()
        .flat_map(|crate_metrics| crate_metrics.sources.iter().cloned())
        .collect::<BTreeSet<_>>();
    for path in inputs {
        let full = root.join(&path);
        let contents = std::fs::read(&full)
            .map_err(|error| format!("unable to read health discovery input {}: {error}", path))?;
        digest_field(&mut hasher, path.as_bytes());
        digest_field(&mut hasher, &contents);
    }
    let discovery = health_discovery_value(sources);
    let discovery_bytes = serde_json::to_vec(&discovery)
        .map_err(|error| format!("unable to serialize health source discovery: {error}"))?;
    digest_field(&mut hasher, &discovery_bytes);
    for path in source_paths {
        let parsed = session
            .rust_file(&root.join(&path))
            .map_err(|error| format!("unable to retrieve cached Rust source {path}: {error}"))?;
        digest_field(&mut hasher, path.as_bytes());
        digest_field(&mut hasher, parsed.source.as_bytes());
    }
    let policy_bytes = serde_json::to_vec(policy)
        .map_err(|error| format!("unable to serialize effective health policy: {error}"))?;
    digest_field(&mut hasher, &policy_bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn health_discovery_value(sources: &[HealthSourceCrate]) -> Value {
    let mut crates = sources
        .iter()
        .map(|crate_source| {
            let mut targets = crate_source
                .targets
                .iter()
                .map(|target| json!({"kind": target.kind, "path": target.path}))
                .collect::<Vec<_>>();
            targets.sort_by(|left, right| {
                left.get("path")
                    .and_then(Value::as_str)
                    .cmp(&right.get("path").and_then(Value::as_str))
                    .then(
                        left.get("kind")
                            .and_then(Value::as_str)
                            .cmp(&right.get("kind").and_then(Value::as_str)),
                    )
            });
            json!({
                "name": crate_source.name,
                "manifest_path": crate_source.manifest_path,
                "targets": targets,
            })
        })
        .collect::<Vec<_>>();
    crates.sort_by(|left, right| {
        left.get("manifest_path")
            .and_then(Value::as_str)
            .cmp(&right.get("manifest_path").and_then(Value::as_str))
    });
    json!(crates)
}

fn digest_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn health_snapshot(
    crates: &[bloomery_check::HealthCrateMetrics],
    scores: &[bloomery_check::health::CrateScore],
    workspace: Option<u16>,
    policy: Value,
    input_digest: String,
) -> Value {
    let crate_records = crates
        .iter()
        .zip(scores)
        .map(|(crate_metrics, score)| {
            let callables = crate_metrics
                .callables
                .iter()
                .map(|callable| {
                    json!({
                        "identity": callable.identity,
                        "path": callable.path,
                        "start": {"line": callable.start_line, "column": callable.start_column},
                        "end": {"line": callable.end_line, "column": callable.end_column},
                        "cyclomatic": callable.cyclomatic,
                        "nesting": callable.nesting,
                        "body_tokens": callable.body_tokens.len(),
                    })
                })
                .collect::<Vec<_>>();
            let ranges = crate_metrics
                .callables
                .iter()
                .filter(|callable| callable.duplicate_token_count > 0)
                .map(|callable| {
                    json!({
                        "path": callable.path,
                        "start_line": callable.start_line,
                        "end_line": callable.end_line,
                        "covered_tokens": callable.duplicate_token_count,
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "manifest_path": crate_metrics.manifest_path,
                "name": crate_metrics.name,
                "sources": crate_metrics.sources,
                "opaque_sources": crate_metrics.opaque_sources.iter().map(|item| json!({
                    "path": item.path,
                    "line": item.line,
                    "description": item.description,
                })).collect::<Vec<_>>(),
                "sloc": crate_metrics.sloc,
                "callables": callables,
                "duplication": {
                    "covered_tokens": crate_metrics.duplicate_tokens,
                    "total_tokens": crate_metrics.callable_tokens,
                    "fraction": score.duplication_fraction,
                    "ranges": ranges,
                },
                "penalties": {
                    "complexity": score.penalties.complexity,
                    "size": score.penalties.size,
                    "nesting": score.penalties.nesting,
                    "duplication": score.penalties.duplication,
                    "crateSize": score.penalties.crate_size,
                },
                "score_hundredths": score.score_hundredths,
                "applicability": if score.score_hundredths.is_some() { "applicable" } else { "not_applicable" },
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": 1,
        "algorithm_version": 2,
        "policy": policy,
        "input_digest": input_digest,
        "crates": crate_records,
        "workspace": {
            "total_sloc": crates.iter().map(|crate_metrics| crate_metrics.sloc).sum::<usize>(),
            "applicable_crates": scores.iter().filter(|score| score.score_hundredths.is_some()).count(),
            "score_hundredths": workspace,
        }
    })
}

#[derive(Debug)]
struct ValidatedHealthBaseline {
    crates: BTreeMap<String, Option<u16>>,
    workspace_score_hundredths: Option<u16>,
}

fn validate_health_baseline(
    baseline: &Value,
    current: &Value,
) -> Result<ValidatedHealthBaseline, String> {
    if baseline.get("schema_version").and_then(Value::as_u64) != Some(4) {
        return Err("baseline has an unsupported schema version".to_owned());
    }
    if baseline.get("algorithm_version").and_then(Value::as_u64) != Some(2) {
        return Err("baseline has an unsupported health algorithm version".to_owned());
    }
    if baseline.pointer("/policy/scoring") != current.pointer("/policy/scoring")
        || baseline.pointer("/policy/selection") != current.pointer("/policy/selection")
    {
        return Err("baseline scoring limits or source-selection policy do not match".to_owned());
    }
    let digest = baseline
        .get("input_digest")
        .and_then(Value::as_str)
        .unwrap_or("");
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("baseline has an invalid input digest".to_owned());
    }
    let crate_records = baseline
        .get("crates")
        .and_then(Value::as_array)
        .ok_or_else(|| "baseline has no crate array".to_owned())?;
    let mut scores = BTreeMap::new();
    let mut total_sloc = 0_u128;
    let mut weighted_score = 0_u128;
    for crate_record in crate_records {
        let manifest = crate_record
            .get("manifest_path")
            .and_then(Value::as_str)
            .filter(|manifest| !manifest.is_empty())
            .ok_or_else(|| "baseline crate is missing manifest_path".to_owned())?;
        let sloc = crate_record
            .get("sloc")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("baseline crate '{manifest}' has no valid SLOC"))?;
        let raw_score = crate_record
            .get("overall_score")
            .ok_or_else(|| format!("baseline crate '{manifest}' has no overall score"))?;
        let score = parse_baseline_score(raw_score)
            .ok_or_else(|| format!("baseline crate '{manifest}' has an invalid overall score"))?;
        if (sloc == 0) != score.is_none() {
            return Err(format!(
                "baseline crate '{manifest}' has inconsistent score applicability"
            ));
        }
        total_sloc += u128::from(sloc);
        if let Some(score) = score {
            weighted_score += u128::from(score) * u128::from(sloc);
        }
        if scores.insert(manifest.to_owned(), score).is_some() {
            return Err(format!(
                "baseline has duplicate crate identity '{manifest}'"
            ));
        }
    }
    let derived_workspace_score = (weighted_score + total_sloc / 2)
        .checked_div(total_sloc)
        .map(|score| score as u16);
    let workspace = baseline
        .get("workspace")
        .ok_or_else(|| "baseline has no workspace summary".to_owned())?;
    if workspace.get("sloc").and_then(Value::as_u64) != u64::try_from(total_sloc).ok() {
        return Err("baseline workspace SLOC is inconsistent with crate rows".to_owned());
    }
    let workspace_score = parse_baseline_score(
        workspace
            .get("overall_score")
            .ok_or_else(|| "baseline workspace has no overall score".to_owned())?,
    )
    .ok_or_else(|| "baseline workspace score is invalid".to_owned())?;
    if workspace_score != derived_workspace_score {
        return Err("baseline workspace score is inconsistent with crate rows".to_owned());
    }
    Ok(ValidatedHealthBaseline {
        crates: scores,
        workspace_score_hundredths: workspace_score,
    })
}

fn parse_baseline_score(value: &Value) -> Option<Option<u16>> {
    if value.is_null() {
        return Some(None);
    }
    let score = value.as_f64()?;
    let hundredths = (score * 100.0).round();
    if !score.is_finite()
        || !(0.0..=100.0).contains(&score)
        || (score * 100.0 - hundredths).abs() > 1e-7
    {
        return None;
    }
    Some(Some(hundredths as u16))
}

#[cfg(test)]
mod health_baseline_tests {
    use super::validate_health_baseline;
    use serde_json::{Value, json};

    fn valid_snapshot() -> Value {
        json!({
            "schema_version": 4,
            "algorithm_version": 2,
            "input_digest": "a".repeat(64),
            "policy": {
                "scoring": {
                    "cyclomaticLimit": 10,
                    "crateSlocLimit": 1500,
                    "weights": {
                        "complexity": 25,
                        "size": 20,
                        "nesting": 10,
                        "duplication": 15,
                        "crateSize": 30
                    }
                },
                "selection": {"include": "production"}
            },
            "crates": [{
                "manifest_path": "crates/demo/Cargo.toml",
                "sloc": 10,
                "overall_score": 100.0
            }],
            "workspace": {
                "sloc": 10,
                "overall_score": 100.0
            }
        })
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-030"))]
    fn baseline_validation_rejects_duplicate_and_inconsistent_records() {
        let valid = valid_snapshot();
        let baseline = validate_health_baseline(&valid, &valid).expect("valid baseline");
        assert_eq!(baseline.crates.len(), 1);
        assert_eq!(baseline.workspace_score_hundredths, Some(10_000));

        let mut duplicate = valid.clone();
        let original = duplicate["crates"][0].clone();
        duplicate["crates"]
            .as_array_mut()
            .expect("crate array")
            .push(original);
        assert!(
            validate_health_baseline(&duplicate, &valid)
                .expect_err("duplicate identity")
                .contains("duplicate crate identity")
        );

        let mut invalid_score = valid.clone();
        invalid_score["crates"][0]["overall_score"] = json!(100.01);
        assert!(
            validate_health_baseline(&invalid_score, &valid)
                .expect_err("out-of-range score")
                .contains("invalid overall score")
        );

        let mut inconsistent_crate = valid.clone();
        inconsistent_crate["crates"][0]["sloc"] = json!(0);
        assert!(
            validate_health_baseline(&inconsistent_crate, &valid)
                .expect_err("inconsistent score applicability")
                .contains("inconsistent score applicability")
        );

        let mut inconsistent_workspace = valid.clone();
        inconsistent_workspace["workspace"]["overall_score"] = json!(99.99);
        assert!(
            validate_health_baseline(&inconsistent_workspace, &valid)
                .expect_err("inconsistent workspace score")
                .contains("workspace score is inconsistent")
        );
    }
}
