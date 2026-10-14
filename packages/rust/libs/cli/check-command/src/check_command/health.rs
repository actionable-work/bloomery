use super::command::{
    CheckContext, CheckError, HealthArgs, HealthUpdateArgs, report_error, write_page,
};
use super::model::{Outcome, RunStatus};
use super::{execution, output, store};
use bloomery_model::health::HealthConfig;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

pub(super) fn run_retrieve(
    context: &CheckContext<'_>,
    args: HealthArgs,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let store = match store::RunStore::open(context.root, context.cache_base) {
        Ok(store) => store,
        Err(message) => {
            return report_error(
                context.json_mode,
                &CheckError::operational(message),
                args.run.as_deref(),
                stdout,
                stderr,
            );
        }
    };
    let loaded = match load_run(&store, args.run.as_deref()) {
        Ok(loaded) => loaded,
        Err(message) => {
            return report_error(
                context.json_mode,
                &CheckError::retrieval(message),
                args.run.as_deref(),
                stdout,
                stderr,
            );
        }
    };
    let report = match read_health_report(&loaded.1) {
        Ok(report) => report,
        Err(message) => {
            return report_error(
                context.json_mode,
                &CheckError::retrieval(message),
                Some(&loaded.0.id),
                stdout,
                stderr,
            );
        }
    };
    let page = match output::health_page(&report, &loaded.0.id, args.offset, args.limit) {
        Ok(page) => page,
        Err(message) => {
            return report_error(
                context.json_mode,
                &CheckError::retrieval(message),
                Some(&loaded.0.id),
                stdout,
                stderr,
            );
        }
    };
    let bytes = if context.json_mode {
        page.bytes
    } else {
        health_page_text(&page.value).into_bytes()
    };
    write_page(context.json_mode, context.color, bytes, stdout, stderr)
}

pub(super) fn run_update(
    context: &CheckContext<'_>,
    args: HealthUpdateArgs,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let prepared = match prepare_health_update(context, args.run.as_deref()) {
        Ok(prepared) => prepared,
        Err(failure) => return render_update_failure(context, failure, stdout, stderr),
    };
    if let Err(message) = atomic_replace(&prepared.destination, &prepared.bytes) {
        return render_update_failure(
            context,
            UpdateFailure::Rejected {
                code: "HealthExportFailed",
                message,
                run: prepared.record.id.clone(),
            },
            stdout,
            stderr,
        );
    }
    render_update_success(context, prepared, stdout, stderr)
}

struct PreparedHealthUpdate {
    record: super::model::RunRecord,
    report: Value,
    destination: PathBuf,
    bytes: Vec<u8>,
}

enum UpdateFailure {
    Operational(String),
    Retrieval {
        message: String,
        run: Option<String>,
    },
    Rejected {
        code: &'static str,
        message: String,
        run: String,
    },
}

fn prepare_health_update(
    context: &CheckContext<'_>,
    selected_run: Option<&str>,
) -> Result<PreparedHealthUpdate, UpdateFailure> {
    let store = store::RunStore::open(context.root, context.cache_base)
        .map_err(UpdateFailure::Operational)?;
    let (record, directory) =
        load_run(&store, selected_run).map_err(|message| UpdateFailure::Retrieval {
            message,
            run: selected_run.map(str::to_owned),
        })?;
    require_exportable_run(&record)?;
    let report = read_health_report(&directory).map_err(|message| UpdateFailure::Rejected {
        code: "HealthExportUnavailable",
        message,
        run: record.id.clone(),
    })?;
    let (policy, effective_policy) = current_health_policy(context.root, &record.id)?;
    validate_report_freshness(context, &record.id, &report, &effective_policy)?;
    let destination =
        validate_destination(context.root, &policy.baseline, &report, &effective_policy).map_err(
            |message| UpdateFailure::Rejected {
                code: "HealthBaselinePathRejected",
                message,
                run: record.id.clone(),
            },
        )?;
    let bytes = serialize_summary(&report).map_err(|message| UpdateFailure::Rejected {
        code: "HealthExportFailed",
        message,
        run: record.id.clone(),
    })?;
    Ok(PreparedHealthUpdate {
        record,
        report,
        destination,
        bytes,
    })
}

fn require_exportable_run(record: &super::model::RunRecord) -> Result<(), UpdateFailure> {
    let formatter_passed = record
        .outcomes
        .iter()
        .find(|outcome| outcome.id == "format:workspace")
        .is_some_and(|outcome| outcome.outcome == Outcome::Passed);
    if matches!(record.status, RunStatus::Passed | RunStatus::Failed) && formatter_passed {
        return Ok(());
    }
    Err(UpdateFailure::Rejected {
        code: "HealthExportUnavailable",
        message: "baseline export requires a finalized uninterrupted run with a passed formatter"
            .to_owned(),
        run: record.id.clone(),
    })
}

fn current_health_policy(root: &Path, run: &str) -> Result<(HealthConfig, Value), UpdateFailure> {
    let loaded = bloomery_model::config::load_with_raw(root)
        .map_err(|diagnostic| stale_export(run, diagnostic.message))?;
    let policy = HealthConfig::from_raw(loaded.raw.as_ref())
        .map_err(|diagnostic| stale_export(run, diagnostic.message))?;
    let raw_json = loaded
        .raw
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|error| stale_export(run, error.to_string()))?;
    let effective_policy = execution::health_policy_value(&policy, &raw_json);
    Ok((policy, effective_policy))
}

fn validate_report_freshness(
    context: &CheckContext<'_>,
    run: &str,
    report: &Value,
    effective_policy: &Value,
) -> Result<(), UpdateFailure> {
    if report.get("policy") != Some(effective_policy) {
        return Err(stale_export(
            run,
            "health configuration changed since the retained measurement",
        ));
    }
    let sources = current_health_sources(context, run)?;
    let current_digest = current_input_digest(context.root, report, &sources, effective_policy)
        .map_err(|message| stale_export(run, message))?;
    if report.get("input_digest").and_then(Value::as_str) == Some(&current_digest) {
        return Ok(());
    }
    Err(stale_export(
        run,
        "source bytes, discovery inputs, inventory, or health configuration changed since the retained measurement",
    ))
}

fn current_health_sources(
    context: &CheckContext<'_>,
    run: &str,
) -> Result<Vec<bloomery_check::HealthSourceCrate>, UpdateFailure> {
    let system = context
        .backend
        .host_system(context.root)
        .map_err(|message| stale_export(run, message))?;
    context
        .backend
        .health_sources(context.root, &system)
        .map_err(|message| stale_export(run, message))
}

fn stale_export(run: &str, message: impl Into<String>) -> UpdateFailure {
    UpdateFailure::Rejected {
        code: "HealthExportStale",
        message: message.into(),
        run: run.to_owned(),
    }
}

fn serialize_summary(report: &Value) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(&health_summary(report))
        .map_err(|error| format!("unable to serialize code health summary: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn render_update_failure(
    context: &CheckContext<'_>,
    failure: UpdateFailure,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    match failure {
        UpdateFailure::Operational(message) => report_error(
            context.json_mode,
            &CheckError::operational(message),
            None,
            stdout,
            stderr,
        ),
        UpdateFailure::Retrieval { message, run } => report_error(
            context.json_mode,
            &CheckError::retrieval(message),
            run.as_deref(),
            stdout,
            stderr,
        ),
        UpdateFailure::Rejected { code, message, run } => update_error(
            context.json_mode,
            code,
            &message,
            Some(&run),
            ExitCode::from(1),
            stdout,
            stderr,
        ),
    }
}

fn render_update_success(
    context: &CheckContext<'_>,
    prepared: PreparedHealthUpdate,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let relative = prepared
        .destination
        .strip_prefix(context.root)
        .unwrap_or(&prepared.destination)
        .to_string_lossy();
    let response = json!({
        "run": prepared.record.id,
        "path": relative,
        "crates": prepared.report.get("crates").and_then(Value::as_array).map_or(0, Vec::len),
        "workspace_score_hundredths": prepared.report.pointer("/workspace/score_hundredths"),
    });
    let bytes = if context.json_mode {
        output::compact_json(&crate::output::success_envelope(
            "check",
            Some("health_update"),
            crate::output::Status::Succeeded,
            response,
            Vec::new(),
        ))
        .unwrap_or_default()
    } else {
        format!(
            "updated {} from run {} · workspace score {}\n",
            relative,
            prepared.record.id,
            display_score(prepared.report.pointer("/workspace/score_hundredths")),
        )
        .into_bytes()
    };
    write_page(context.json_mode, context.color, bytes, stdout, stderr)
}

fn load_run(
    store: &store::RunStore,
    selected: Option<&str>,
) -> Result<(super::model::RunRecord, PathBuf), String> {
    match selected {
        Some(run) => store.load(run),
        None => store.latest(),
    }
}

fn read_health_report(directory: &Path) -> Result<Value, String> {
    let bytes = fs::read(directory.join("health-report.json"))
        .map_err(|error| format!("no complete health report is retained for this run: {error}"))?;
    let report: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("retained health report is malformed: {error}"))?;
    if report.get("schema_version").and_then(Value::as_u64) != Some(1)
        || report.get("algorithm_version").and_then(Value::as_u64) != Some(1)
    {
        return Err(
            "retained health report uses an unsupported schema or algorithm version".to_owned(),
        );
    }
    Ok(report)
}

fn health_page_text(page: &Value) -> String {
    let mut text = format!(
        "workspace score {} · baseline {} · delta {}\n",
        display_score(page.get("workspace_score_hundredths")),
        display_score(page.get("baseline_workspace_score_hundredths")),
        display_delta(page.get("workspace_delta_hundredths")),
    );
    let offset = page.get("offset").and_then(Value::as_u64).unwrap_or(0);
    let total = page.get("total").and_then(Value::as_u64).unwrap_or(0);
    text.push_str(&format!(
        "crates {}–{} of {}\n",
        offset + 1,
        offset + page["crates"].as_array().map_or(0, Vec::len) as u64,
        total
    ));
    if let Some(crates) = page.get("crates").and_then(Value::as_array) {
        for crate_record in crates {
            text.push_str(&format!(
                "{} · {} · delta {}\n",
                crate_record
                    .get("manifest_path")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                display_score(crate_record.get("score_hundredths")),
                display_delta(crate_record.get("delta_hundredths")),
            ));
        }
    }
    if let Some(next) = page.get("next").and_then(Value::as_u64) {
        text.push_str(&format!("next offset {next}\n"));
    }
    text
}

fn display_score(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_u64)
        .map(|score| format!("{:.2}", score as f64 / 100.0))
        .unwrap_or_else(|| "n/a".to_owned())
}

fn display_delta(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_i64)
        .map(|delta| format!("{:+.2}", delta as f64 / 100.0))
        .unwrap_or_else(|| "n/a".to_owned())
}

fn current_input_digest(
    root: &Path,
    report: &Value,
    sources: &[bloomery_check::HealthSourceCrate],
    policy: &Value,
) -> Result<String, String> {
    let mut hasher = Sha256::new();
    let selected_manifest = policy
        .pointer("/selection/manifest")
        .and_then(Value::as_str)
        .unwrap_or("Cargo.toml");
    let mut inputs = BTreeSet::from([
        selected_manifest.to_owned(),
        ".bloomery/config.toml".to_owned(),
    ]);
    for crate_source in sources {
        inputs.insert(crate_source.manifest_path.clone());
    }
    for path in inputs {
        let bytes = fs::read(root.join(&path))
            .map_err(|error| format!("unable to recheck discovery input {path}: {error}"))?;
        digest_field(&mut hasher, path.as_bytes());
        digest_field(&mut hasher, &bytes);
    }
    let discovery = execution::health_discovery_value(sources);
    let discovery_bytes = serde_json::to_vec(&discovery)
        .map_err(|error| format!("unable to serialize current source discovery: {error}"))?;
    digest_field(&mut hasher, &discovery_bytes);
    let source_paths = report
        .get("crates")
        .and_then(Value::as_array)
        .ok_or_else(|| "retained report has no crate source inventory".to_owned())?
        .iter()
        .flat_map(|crate_record| {
            crate_record
                .get("sources")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
        })
        .collect::<BTreeSet<_>>();
    for path in source_paths {
        let bytes = fs::read(root.join(&path))
            .map_err(|error| format!("unable to recheck analyzed Rust source {path}: {error}"))?;
        digest_field(&mut hasher, path.as_bytes());
        digest_field(&mut hasher, &bytes);
    }
    let policy_bytes = serde_json::to_vec(policy)
        .map_err(|error| format!("unable to serialize current health policy: {error}"))?;
    digest_field(&mut hasher, &policy_bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn digest_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn health_summary(report: &Value) -> Value {
    let mut crates = report
        .get("crates")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(summary_crate)
        .collect::<Vec<_>>();
    crates.sort_by(|left, right| {
        left["manifest_path"]
            .as_str()
            .cmp(&right["manifest_path"].as_str())
    });
    json!({
        "schema_version": 3,
        "algorithm_version": report.get("algorithm_version"),
        "policy": {
            "scoring": report.pointer("/policy/scoring"),
            "selection": report.pointer("/policy/selection"),
        },
        "input_digest": report.get("input_digest"),
        "workspace": {
            "crate_count": report.get("crates").and_then(Value::as_array).map_or(0, Vec::len),
            "sloc": report.pointer("/workspace/total_sloc"),
            "overall_score": score_value(report.pointer("/workspace/score_hundredths")),
            "subscores": workspace_subscores(report),
        },
        "crates": crates,
    })
}

fn summary_crate(crate_record: &Value) -> Value {
    json!({
        "manifest_path": crate_record.get("manifest_path"),
        "name": crate_record.get("name"),
        "sloc": crate_record.get("sloc"),
        "overall_score": score_value(crate_record.get("score_hundredths")),
        "subscores": rounded_penalties(crate_record.get("penalties")),
    })
}

fn score_value(score: Option<&Value>) -> Value {
    score
        .and_then(Value::as_u64)
        .map(|hundredths| json!(hundredths as f64 / 100.0))
        .unwrap_or(Value::Null)
}

fn rounded_penalties(penalties: Option<&Value>) -> Value {
    let rounded = |name: &str| {
        let value = penalties
            .and_then(|value| value.get(name))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        (value * 100.0).round() / 100.0
    };
    json!({
        "complexity": rounded("complexity"),
        "size": rounded("size"),
        "nesting": rounded("nesting"),
        "duplication": rounded("duplication"),
    })
}

fn workspace_subscores(report: &Value) -> Value {
    let total_sloc = report
        .pointer("/workspace/total_sloc")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let weighted_penalty = |name: &str| {
        if total_sloc == 0 {
            return 0.0;
        }
        let weighted = report
            .get("crates")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|crate_record| {
                let sloc = crate_record
                    .get("sloc")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let penalty = crate_record
                    .pointer(&format!("/penalties/{name}"))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                penalty * sloc as f64
            })
            .sum::<f64>()
            / total_sloc as f64;
        (weighted * 100.0).round() / 100.0
    };
    json!({
        "complexity": weighted_penalty("complexity"),
        "size": weighted_penalty("size"),
        "nesting": weighted_penalty("nesting"),
        "duplication": weighted_penalty("duplication"),
    })
}

fn validate_destination(
    root: &Path,
    baseline: &str,
    report: &Value,
    policy: &Value,
) -> Result<PathBuf, String> {
    let root = fs::canonicalize(root)
        .map_err(|error| format!("unable to resolve repository root: {error}"))?;
    let relative = Path::new(baseline);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("baseline destination must stay inside the repository".to_owned());
    }
    let destination = root.join(relative);
    let parent = destination
        .parent()
        .ok_or_else(|| "baseline destination has no parent".to_owned())?;
    let mut current = root.clone();
    for component in parent
        .strip_prefix(&root)
        .map_err(|_| "baseline destination escapes the repository".to_owned())?
        .components()
    {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let resolved = fs::canonicalize(&current).map_err(|error| {
                    format!("unable to resolve baseline parent symlink: {error}")
                })?;
                if !resolved.starts_with(&root) {
                    return Err("baseline parent symlink escapes the repository".to_owned());
                }
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err("baseline parent component is not a directory".to_owned());
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => break,
            Err(error) => return Err(format!("unable to inspect baseline parent: {error}")),
        }
    }
    if let Ok(metadata) = fs::symlink_metadata(&destination)
        && (metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err("baseline destination must be a regular repository file".to_owned());
    }
    let mut protected = BTreeSet::from([
        root.join(".bloomery/config.toml"),
        root.join("Cargo.lock"),
        root.join("flake.lock"),
        root.join(".bloomery/specs"),
    ]);
    if let Some(manifest) = policy
        .pointer("/selection/manifest")
        .and_then(Value::as_str)
    {
        protected.insert(root.join(manifest));
    }
    if let Some(crate_records) = report.get("crates").and_then(Value::as_array) {
        for crate_record in crate_records {
            if let Some(manifest) = crate_record.get("manifest_path").and_then(Value::as_str) {
                protected.insert(root.join(manifest));
            }
            if let Some(sources) = crate_record.get("sources").and_then(Value::as_array) {
                for source in sources.iter().filter_map(Value::as_str) {
                    protected.insert(root.join(source));
                }
            }
        }
    }
    if protected
        .iter()
        .any(|path| destination.starts_with(path) || path.starts_with(&destination))
    {
        return Err("baseline destination overlaps a manifest, source, configuration, lockfile, or specification".to_owned());
    }
    Ok(destination)
}

fn atomic_replace(destination: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = destination
        .parent()
        .ok_or_else(|| "baseline destination has no parent".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("unable to create baseline directory: {error}"))?;
    let file_name = destination
        .file_name()
        .ok_or_else(|| "baseline destination has no file name".to_owned())?
        .to_string_lossy();
    let temporary = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o644);
    }
    let result = (|| {
        let mut file = options
            .open(&temporary)
            .map_err(|error| format!("unable to create temporary baseline: {error}"))?;
        file.write_all(bytes)
            .map_err(|error| format!("unable to write temporary baseline: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("unable to sync temporary baseline: {error}"))?;
        fs::rename(&temporary, destination)
            .map_err(|error| format!("unable to atomically replace baseline: {error}"))?;
        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("unable to sync baseline directory: {error}"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn update_error(
    json_mode: bool,
    code: &str,
    message: &str,
    run: Option<&str>,
    status: ExitCode,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let envelope = crate::output::error_envelope(
        Some("check"),
        Some("health_update"),
        crate::output::Status::Failed,
        code,
        "check",
        message,
        run.map(|run| json!({"run": run})),
    );
    let bytes = if json_mode {
        output::compact_json(&envelope).unwrap_or_default()
    } else {
        format!("bloomery: {message}\n").into_bytes()
    };
    let _ = write_page(
        json_mode,
        crate::output::ColorMode::Never,
        bytes,
        stdout,
        stderr,
    );
    status
}
