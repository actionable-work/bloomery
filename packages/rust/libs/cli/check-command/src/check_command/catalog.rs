use super::interrupt::CancellationToken;
use super::model::{Notice, valid_system_name};
use super::nix_progress::NixProgressCollector;
use super::progress::ProgressSink;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::Duration;

const LEGACY_CHECK_ATTRIBUTE: &str = "bloomery:check";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NixTaskResult<T> {
    Succeeded(T),
    Failed { code: String, message: String },
    Canceled,
    OperationalError(String),
}

/// One discovered check attribute and the evaluation result for its derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckPlan {
    pub attribute: String,
    pub derivation: Option<String>,
    pub eval_error: Option<PlanError>,
    pub outputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanError {
    pub code: String,
    pub message: String,
}

/// A complete catalog evaluation for one selected system. `dependencies` maps
/// every known derivation store path to its direct input derivations so the
/// runner can attribute dependency-only failures without re-evaluating.
/// Maps derivation store paths to their output paths and direct input
/// derivations: `(outputs_by_derivation, dependencies_by_derivation)`.
type PlanMetadata = (BTreeMap<String, Vec<String>>, BTreeMap<String, Vec<String>>);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlannedSystem {
    pub legacy: bool,
    pub plans: Vec<CheckPlan>,
    pub dependencies: BTreeMap<String, Vec<String>>,
}

/// Retained failure evidence for one failed derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureBlock {
    pub derivation: String,
    pub code: String,
    pub message: String,
    pub log_hint: Option<String>,
    pub excerpt: Vec<String>,
}

/// Result of one batched realization invocation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BatchRealization {
    pub failures: BTreeMap<String, FailureBlock>,
    pub canceled: bool,
    pub operational_error: Option<String>,
}

pub trait NixBackend: Send + Sync {
    fn host_system(&self, root: &Path) -> Result<String, String>;
    fn discover(&self, root: &Path, system: &str) -> Result<Vec<String>, String>;
    /// Evaluate the whole check catalog for one system in a single Nix
    /// evaluation, including per-attribute derivation paths and dependency
    /// metadata.
    fn plan_checks(&self, root: &Path, system: &str) -> Result<PlannedSystem, String>;
    /// Run the flake's default formatter once from the workspace root. The
    /// caller supplies a run-local log path that receives all command output.
    fn format_workspace(
        &self,
        root: &Path,
        log_path: &Path,
        cancellation: CancellationToken,
    ) -> NixTaskResult<()>;
    /// Realize every planned derivation in one `nix build` invocation. The
    /// caller supplies the retained batch log path and the live progress sink.
    fn realize_batch(
        &self,
        root: &Path,
        plans: &[CheckPlan],
        fail_fast: bool,
        log_path: &Path,
        cancellation: CancellationToken,
        progress: &ProgressSink,
    ) -> BatchRealization;
    /// Resolve which of the supplied output store paths are valid after
    /// realization in one structured query.
    fn validate_outputs(
        &self,
        root: &Path,
        outputs: &[String],
    ) -> Result<std::collections::BTreeSet<String>, String>;
    fn read_derivation_log(&self, root: &Path, store_path: &str) -> Result<Vec<u8>, String>;
}

#[derive(Debug, Clone)]
pub struct NixCli {
    executable: OsString,
}

impl Default for NixCli {
    fn default() -> Self {
        Self {
            executable: OsString::from("nix"),
        }
    }
}

struct LoggedCommand<'a> {
    root: &'a Path,
    args: &'a [OsString],
    log_path: &'a Path,
    cancellation: CancellationToken,
    capture_stdout: bool,
    failure_code: Option<&'a str>,
    failure_description: &'a str,
    progress: Option<&'a NixProgressCollector>,
}

impl NixCli {
    #[cfg(test)]
    pub fn with_executable(executable: impl Into<OsString>) -> Self {
        Self {
            executable: executable.into(),
        }
    }

    fn run_catalog_command(
        &self,
        root: &Path,
        args: &[OsString],
        operation: &str,
    ) -> Result<Vec<u8>, String> {
        let output = Command::new(&self.executable)
            .args(args)
            .current_dir(root)
            .output()
            .map_err(|error| nix_start_error(operation, error))?;
        if !output.status.success() {
            let detail = bounded_process_error(&output.stderr);
            return Err(if detail.is_empty() {
                format!("Nix {operation} failed with {}", status_text(output.status))
            } else {
                format!("Nix {operation} failed: {detail}")
            });
        }
        Ok(output.stdout)
    }

    fn read_derivation_log(&self, root: &Path, store_path: &str) -> Result<Vec<u8>, String> {
        if !valid_nix_derivation_path(store_path) {
            return Err("invalid retained Nix derivation path".to_owned());
        }
        let output = Command::new(&self.executable)
            .arg("log")
            .arg(store_path)
            .current_dir(root)
            .output()
            .map_err(|error| nix_start_error("derivation log retrieval", error))?;
        if !output.status.success() {
            let detail = bounded_process_error(&output.stderr);
            return Err(if detail.is_empty() {
                format!("Nix log failed with {}", status_text(output.status))
            } else {
                format!("Nix log failed: {detail}")
            });
        }
        let mut log = output.stdout;
        if !output.stderr.is_empty() {
            log.extend_from_slice(b"\n");
            log.extend_from_slice(&output.stderr);
        }
        Ok(log)
    }

    fn run_logged_command(&self, command: LoggedCommand<'_>) -> NixTaskResult<Vec<u8>> {
        let LoggedCommand {
            root,
            args,
            log_path,
            cancellation,
            capture_stdout,
            failure_code,
            failure_description,
            progress,
        } = command;
        let mut log = match open_private_log(log_path) {
            Ok(file) => file,
            Err(error) => {
                return NixTaskResult::OperationalError(format!(
                    "unable to open retained task log: {error}"
                ));
            }
        };
        let collector = progress.cloned().filter(NixProgressCollector::is_active);

        let mut command = Command::new(&self.executable);
        command.args(args).current_dir(root);
        if collector.is_some() {
            command.stderr(Stdio::piped());
        } else {
            let stderr = match log.try_clone() {
                Ok(file) => file,
                Err(error) => {
                    return NixTaskResult::OperationalError(format!(
                        "unable to prepare retained task log: {error}"
                    ));
                }
            };
            command.stderr(Stdio::from(stderr));
        }
        if capture_stdout {
            command.stdout(Stdio::piped());
        } else {
            match log.try_clone() {
                Ok(file) => {
                    command.stdout(Stdio::from(file));
                }
                Err(error) => {
                    return NixTaskResult::OperationalError(format!(
                        "unable to prepare retained task log: {error}"
                    ));
                }
            }
        }

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                return NixTaskResult::OperationalError(nix_start_error_message(error));
            }
        };

        let stdout_reader = if capture_stdout {
            child.stdout.take().map(|mut stdout| {
                thread::spawn(move || {
                    let mut bytes = Vec::new();
                    let result = stdout.read_to_end(&mut bytes);
                    (bytes, result)
                })
            })
        } else {
            None
        };

        let stderr_reader = match (collector, child.stderr.take()) {
            (Some(collector), Some(pipe)) => match log.try_clone() {
                Ok(log_for_stderr) => Some(thread::spawn(move || {
                    drain_progress_stderr(pipe, log_for_stderr, &collector)
                })),
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return NixTaskResult::OperationalError(format!(
                        "unable to prepare retained task log: {error}"
                    ));
                }
            },
            _ => None,
        };

        let status = loop {
            if cancellation.is_canceled() {
                let _ = child.kill();
                let _ = child.wait();
                let stderr_result = join_progress_stderr(stderr_reader);
                let captured = join_stdout(stdout_reader);
                if let Some(bytes) = captured {
                    let _ = log.write_all(&bytes);
                }
                let _ = log.flush();
                if let Err(error) = stderr_result {
                    return NixTaskResult::OperationalError(format!(
                        "unable to retain Nix task log: {error}"
                    ));
                }
                return NixTaskResult::Canceled;
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => thread::sleep(Duration::from_millis(20)),
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = join_progress_stderr(stderr_reader);
                    return NixTaskResult::OperationalError(format!(
                        "unable to wait for Nix task: {error}"
                    ));
                }
            }
        };
        let stderr_result = join_progress_stderr(stderr_reader);

        let stdout = match stdout_reader {
            Some(reader) => match reader.join() {
                Ok((bytes, Ok(_))) => bytes,
                Ok((_bytes, Err(error))) => {
                    return NixTaskResult::OperationalError(format!(
                        "unable to capture Nix output: {error}"
                    ));
                }
                Err(_) => {
                    return NixTaskResult::OperationalError(
                        "Nix output reader terminated unexpectedly".to_owned(),
                    );
                }
            },
            None => Vec::new(),
        };
        if log.write_all(&stdout).is_err() || log.flush().is_err() {
            return NixTaskResult::OperationalError("unable to retain Nix task output".to_owned());
        }
        if let Err(error) = stderr_result {
            return NixTaskResult::OperationalError(format!(
                "unable to retain Nix task log: {error}"
            ));
        }
        if status.success() {
            NixTaskResult::Succeeded(stdout)
        } else if let Some(code) = failure_code {
            NixTaskResult::Failed {
                code: code.to_owned(),
                message: format!("{failure_description} with {}", status_text(status)),
            }
        } else {
            NixTaskResult::OperationalError(format!(
                "{failure_description} with {}",
                status_text(status)
            ))
        }
    }
}

impl NixBackend for NixCli {
    fn host_system(&self, _root: &Path) -> Result<String, String> {
        host_nix_system()
    }

    fn discover(&self, root: &Path, system: &str) -> Result<Vec<String>, String> {
        if !valid_system_name(system) {
            return Err(format!("invalid Nix system name '{system}'"));
        }
        let output = self.run_catalog_command(root, &discover_args(system), "check discovery")?;
        let value: Value = serde_json::from_slice(&output)
            .map_err(|error| format!("Nix returned invalid check catalog JSON: {error}"))?;
        value
            .as_array()
            .ok_or_else(|| "Nix check catalog is not an array".to_owned())?
            .iter()
            .map(|attribute| {
                attribute
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| "Nix check catalog contains a non-string name".to_owned())
            })
            .collect()
    }

    fn read_derivation_log(&self, root: &Path, store_path: &str) -> Result<Vec<u8>, String> {
        NixCli::read_derivation_log(self, root, store_path)
    }

    fn format_workspace(
        &self,
        root: &Path,
        log_path: &Path,
        cancellation: CancellationToken,
    ) -> NixTaskResult<()> {
        let args = format_args();
        match self.run_logged_command(LoggedCommand {
            root,
            args: &args,
            log_path,
            cancellation,
            capture_stdout: false,
            failure_code: Some("NixFormatFailed"),
            failure_description: "nix fmt failed",
            progress: None,
        }) {
            NixTaskResult::Succeeded(_) => NixTaskResult::Succeeded(()),
            NixTaskResult::Failed { code, message } => NixTaskResult::Failed { code, message },
            NixTaskResult::Canceled => NixTaskResult::Canceled,
            NixTaskResult::OperationalError(message) => NixTaskResult::OperationalError(message),
        }
    }

    fn plan_checks(&self, root: &Path, system: &str) -> Result<PlannedSystem, String> {
        if !valid_system_name(system) {
            return Err(format!("invalid Nix system name '{system}'"));
        }
        let output =
            self.run_catalog_command(root, &plan_args(system), "check catalog evaluation")?;
        let value: Value = serde_json::from_slice(&output)
            .map_err(|error| format!("Nix returned invalid check catalog JSON: {error}"))?;
        let legacy = value
            .get("legacy")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let plan_object = value
            .get("plan")
            .and_then(Value::as_object)
            .ok_or_else(|| "Nix check catalog has no plan map".to_owned())?;
        let mut plans = Vec::with_capacity(plan_object.len());
        for (attribute, entry) in plan_object {
            let success = entry
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let derivation = entry
                .get("drvPath")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .filter(|path| valid_nix_derivation_path(path));
            match (success, derivation) {
                (true, Some(derivation)) => plans.push(CheckPlan {
                    attribute: attribute.clone(),
                    derivation: Some(derivation),
                    eval_error: None,
                    outputs: Vec::new(),
                }),
                _ => plans.push(CheckPlan {
                    attribute: attribute.clone(),
                    derivation: None,
                    eval_error: Some(PlanError {
                        code: "EvaluationFailed".to_owned(),
                        message: format!("{attribute} did not evaluate to a derivation"),
                    }),
                    outputs: Vec::new(),
                }),
            }
        }
        let relevant = plans
            .iter()
            .filter_map(|plan| plan.derivation.clone())
            .collect::<Vec<_>>();
        let (outputs, dependencies) = self.load_plan_metadata(root, &relevant)?;
        for plan in &mut plans {
            if let Some(derivation) = &plan.derivation
                && let Some(paths) = outputs.get(derivation)
            {
                plan.outputs = paths.clone();
            }
        }
        plans.sort_by(|left, right| left.attribute.cmp(&right.attribute));
        Ok(PlannedSystem {
            legacy,
            plans,
            dependencies,
        })
    }

    fn validate_outputs(
        &self,
        root: &Path,
        outputs: &[String],
    ) -> Result<BTreeSet<String>, String> {
        if outputs.is_empty() {
            return Ok(BTreeSet::new());
        }
        let mut args = validity_args();
        args.extend(outputs.iter().map(OsString::from));
        let output = self.run_catalog_command(root, &args, "output validity")?;
        let value: Value = serde_json::from_slice(&output)
            .map_err(|error| format!("Nix output validity is not JSON: {error}"))?;
        let map = value
            .as_object()
            .ok_or_else(|| "Nix output validity is not an object".to_owned())?;
        let mut valid = BTreeSet::new();
        for (path, info) in map {
            if !info.is_null() {
                valid.insert(path.clone());
            }
        }
        Ok(valid)
    }

    fn realize_batch(
        &self,
        root: &Path,
        plans: &[CheckPlan],
        fail_fast: bool,
        log_path: &Path,
        cancellation: CancellationToken,
        progress: &ProgressSink,
    ) -> BatchRealization {
        let derivations = plans
            .iter()
            .filter_map(|plan| plan.derivation.clone())
            .collect::<Vec<_>>();
        if derivations.is_empty() {
            return BatchRealization::default();
        }
        let collector = NixProgressCollector::new(progress.clone());
        let active = collector.is_active();
        if active {
            let mut metadata = BTreeMap::new();
            for plan in plans {
                for output in &plan.outputs {
                    if let Some(derivation) = &plan.derivation {
                        metadata.insert(output.clone(), derivation.clone());
                    }
                }
            }
            collector.set_output_metadata(metadata);
        }
        let mut args = batch_build_args(&derivations, fail_fast);
        if active {
            args.push(OsString::from("--log-format"));
            args.push(OsString::from("internal-json"));
        }
        let result = self.run_logged_command(LoggedCommand {
            root,
            args: &args,
            log_path,
            cancellation,
            capture_stdout: false,
            failure_code: Some("NixBatchFailed"),
            failure_description: "batched Nix realization failed",
            progress: active.then_some(&collector),
        });
        let retained = fs::read(log_path).unwrap_or_default();
        match result {
            NixTaskResult::Succeeded(_) => {
                collector.finish_success();
                BatchRealization::default()
            }
            NixTaskResult::Failed { .. } => {
                collector.finish_failure();
                let text = String::from_utf8_lossy(&retained);
                BatchRealization {
                    failures: segment_failure_blocks(&text)
                        .into_iter()
                        .map(|block| (block.derivation.clone(), block))
                        .collect(),
                    canceled: false,
                    operational_error: None,
                }
            }
            NixTaskResult::Canceled => {
                collector.finish_canceled();
                BatchRealization {
                    failures: BTreeMap::new(),
                    canceled: true,
                    operational_error: None,
                }
            }
            NixTaskResult::OperationalError(message) => {
                collector.finish_failure();
                BatchRealization {
                    failures: BTreeMap::new(),
                    canceled: false,
                    operational_error: Some(message),
                }
            }
        }
    }
}

impl NixCli {
    /// Resolve each selected derivation's output paths and direct input
    /// derivations in one recursive metadata query.
    fn load_plan_metadata(
        &self,
        root: &Path,
        derivations: &[String],
    ) -> Result<PlanMetadata, String> {
        if derivations.is_empty() {
            return Ok((BTreeMap::new(), BTreeMap::new()));
        }
        let mut args = metadata_args();
        args.extend(derivations.iter().map(OsString::from));
        let output = self.run_catalog_command(root, &args, "derivation metadata")?;
        let value: Value = serde_json::from_slice(&output)
            .map_err(|error| format!("Nix derivation metadata is not JSON: {error}"))?;
        let entries = value
            .get("derivations")
            .and_then(Value::as_object)
            .ok_or_else(|| "Nix derivation metadata has no derivations map".to_owned())?;
        let mut outputs = BTreeMap::new();
        let mut dependencies = BTreeMap::new();
        for (drv_name, derivation) in entries {
            let drv = full_store_path(drv_name);
            let mut paths = Vec::new();
            if let Some(output_map) = derivation.get("outputs").and_then(Value::as_object) {
                for output in output_map.values() {
                    if let Some(path) = output.get("path").and_then(Value::as_str) {
                        paths.push(full_store_path(path));
                    }
                }
            }
            paths.sort();
            outputs.insert(drv.clone(), paths);
            let input_map = derivation
                .get("inputDrvs")
                .or_else(|| {
                    derivation
                        .get("inputs")
                        .and_then(|inputs| inputs.get("drvs"))
                })
                .and_then(Value::as_object);
            let mut inputs = input_map
                .map(|input_map| {
                    input_map
                        .keys()
                        .map(|key| full_store_path(key))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            inputs.sort();
            dependencies.insert(drv, inputs);
        }
        Ok((outputs, dependencies))
    }
}

fn full_store_path(value: &str) -> String {
    if value.starts_with("/nix/store/") {
        value.to_owned()
    } else {
        format!("/nix/store/{value}")
    }
}

pub fn discover_catalog(
    root: &Path,
    systems: &[String],
    backend: &dyn NixBackend,
) -> Result<(Vec<String>, Vec<Notice>), String> {
    let mut ids = vec![
        "static:structure".to_owned(),
        "static:traceability".to_owned(),
    ];
    let mut notices = Vec::new();
    for system in systems {
        let attributes = backend.discover(root, system)?;
        for attribute in attributes {
            if attribute == LEGACY_CHECK_ATTRIBUTE {
                notices.push(Notice {
                    code: "LegacyRecursiveCheckExcluded".to_owned(),
                    message: format!(
                        "excluded legacy recursive check output for {system}: {LEGACY_CHECK_ATTRIBUTE}"
                    ),
                });
                continue;
            }
            ids.push(format!("nix:{system}:{attribute}"));
        }
    }
    ids.sort();
    ids.dedup();
    notices.sort_by(|left, right| left.message.cmp(&right.message));
    notices.dedup_by(|left, right| left.code == right.code && left.message == right.message);
    Ok((ids, notices))
}

#[cfg(test)]
pub fn parse_nix_id(id: &str) -> Option<(&str, &str)> {
    let value = id.strip_prefix("nix:")?;
    let (system, attribute) = value.split_once(':')?;
    (!system.is_empty() && !attribute.is_empty()).then_some((system, attribute))
}

#[cfg(test)]
pub(super) fn nix_log_store_path(output: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(output);
    let safe = super::details::normalize_display_text(&text);
    let mut expects_log_command = false;
    for line in safe.lines() {
        if line.to_ascii_lowercase().contains("for full logs") {
            if let Some(path) = nix_log_path_from_line(line) {
                return Some(path);
            }
            expects_log_command = true;
            continue;
        }
        if expects_log_command {
            if line.trim().is_empty() {
                continue;
            }
            expects_log_command = false;
            if let Some(path) = nix_log_path_from_line(line) {
                return Some(path);
            }
        }
    }
    None
}

fn nix_log_path_from_line(line: &str) -> Option<String> {
    let (_, command) = line.split_once("nix log")?;
    let command = command.trim_start().trim_start_matches(['\'', '"', '`']);
    let candidate = command
        .split_whitespace()
        .next()?
        .trim_end_matches(['\'', '"', '`', '.', ',', ';', ')', ']']);
    valid_nix_derivation_path(candidate).then(|| candidate.to_owned())
}

/// Segment retained realization output into one failure block per failed
/// derivation so details can be attributed without relying on the localized text.
pub fn segment_failure_blocks(text: &str) -> Vec<FailureBlock> {
    let mut extracted = String::new();
    for line in text.lines() {
        if let Some(payload) = line.strip_prefix("@nix ")
            && let Ok(value) = serde_json::from_str::<Value>(payload)
            && let Some(message) = value.get("msg").and_then(Value::as_str)
        {
            extracted.push_str(message);
            extracted.push('\n');
            continue;
        }
        extracted.push_str(line);
        extracted.push('\n');
    }
    let safe = super::details::normalize_display_text(&extracted);
    let mut blocks: Vec<FailureBlock> = Vec::new();
    for line in safe.lines() {
        let trimmed = line.trim();
        if let Some(derivation) = parse_cannot_build(trimmed) {
            blocks.push(FailureBlock {
                derivation,
                code: "NixCheckFailed".to_owned(),
                message: String::new(),
                log_hint: None,
                excerpt: Vec::new(),
            });
            continue;
        }
        let Some(block) = blocks.last_mut() else {
            continue;
        };
        if block.message.is_empty()
            && let Some(reason) = trimmed.strip_prefix("Reason:")
        {
            block.message = reason.trim().to_owned();
        } else if block.log_hint.is_none()
            && let Some(path) = nix_log_path_from_line(trimmed)
        {
            block.log_hint = Some(path);
        } else if let Some(excerpt) = trimmed.strip_prefix('>') {
            block.excerpt.push(excerpt.trim().to_owned());
        }
    }
    blocks
}

fn parse_cannot_build(line: &str) -> Option<String> {
    let start = line.find("Cannot build '")? + "Cannot build '".len();
    let rest = &line[start..];
    let end = rest.find('\'')?;
    let derivation = &rest[..end];
    valid_nix_derivation_path(derivation).then(|| derivation.to_owned())
}

fn valid_nix_derivation_path(path: &str) -> bool {
    let Some(name) = path.strip_prefix("/nix/store/") else {
        return false;
    };
    !name.is_empty()
        && name.ends_with(".drv")
        && !name.chars().any(|character| {
            character.is_control() || character.is_whitespace() || character == '/'
        })
}

fn eval_base_args() -> Vec<OsString> {
    ["eval", "--no-write-lock-file", "--no-update-lock-file"]
        .into_iter()
        .map(OsString::from)
        .collect()
}

fn discover_args(system: &str) -> Vec<OsString> {
    let mut args = eval_base_args();
    args.extend([
        OsString::from("--json"),
        OsString::from(format!(".#checks.{system}")),
        OsString::from("--apply"),
        OsString::from("builtins.attrNames"),
    ]);
    args
}

fn plan_args(system: &str) -> Vec<OsString> {
    let mut args = eval_base_args();
    args.extend([
        OsString::from("--json"),
        OsString::from(format!(".#checks.{system}")),
        OsString::from("--apply"),
        OsString::from(
            r#"checks:
  let
    legacy = builtins.hasAttr "bloomery:check" checks;
    filtered = builtins.removeAttrs checks [ "bloomery:check" ];
    plan = builtins.mapAttrs (name: value:
      let attempt = builtins.tryEval value.drvPath;
      in if attempt.success
         then { success = true; drvPath = attempt.value; }
         else { success = false; }
    ) filtered;
  in { inherit legacy plan; }"#,
        ),
    ]);
    args
}

fn metadata_args() -> Vec<OsString> {
    ["derivation", "show", "--recursive"]
        .into_iter()
        .map(OsString::from)
        .collect()
}

fn batch_build_args(derivations: &[String], fail_fast: bool) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("build"),
        OsString::from("--no-link"),
        OsString::from("--no-write-lock-file"),
        OsString::from("--no-update-lock-file"),
    ];
    if !fail_fast {
        args.push(OsString::from("--keep-going"));
    }
    args.extend(
        derivations
            .iter()
            .map(|derivation| OsString::from(format!("{derivation}^*"))),
    );
    args
}

fn validity_args() -> Vec<OsString> {
    ["path-info", "--json"]
        .into_iter()
        .map(OsString::from)
        .collect()
}

fn format_args() -> Vec<OsString> {
    ["fmt", "--no-write-lock-file", "--no-update-lock-file"]
        .into_iter()
        .map(OsString::from)
        .collect()
}

fn host_nix_system() -> Result<String, String> {
    if let Some(configured) = configured_nix_system() {
        if valid_system_name(&configured) {
            return Ok(configured);
        }
        return Err(format!("invalid configured Nix system '{configured}'"));
    }
    target_nix_system(std::env::consts::ARCH, std::env::consts::OS).ok_or_else(|| {
        format!(
            "unable to infer the host Nix system for {}-{}; select one with --system",
            std::env::consts::ARCH,
            std::env::consts::OS
        )
    })
}

fn configured_nix_system() -> Option<String> {
    let mut configuration = BTreeMap::new();
    for contents in nix_config_contents() {
        for line in contents.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if let Some((name, value)) = line.split_once('=') {
                configuration.insert(name.trim().to_owned(), value.trim().to_owned());
            }
        }
    }
    let eval_system = configuration
        .get("eval-system")
        .filter(|value| !value.is_empty());
    eval_system
        .or_else(|| configuration.get("system"))
        .map(|value| value.trim_matches('"').to_owned())
}

fn nix_config_contents() -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(directory) = std::env::var_os("NIX_CONF_DIR") {
        paths.push(PathBuf::from(directory).join("nix.conf"));
    } else {
        paths.push(PathBuf::from("/etc/nix/nix.conf"));
    }
    if let Some(directory) = std::env::var_os("XDG_CONFIG_HOME") {
        paths.push(PathBuf::from(directory).join("nix/nix.conf"));
    } else if let Some(home) = std::env::var_os("HOME") {
        paths.push(PathBuf::from(home).join(".config/nix/nix.conf"));
    }
    let mut contents = paths
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .collect::<Vec<_>>();
    if let Ok(environment) = std::env::var("NIX_CONFIG") {
        contents.push(environment);
    }
    contents
}

fn target_nix_system(architecture: &str, operating_system: &str) -> Option<String> {
    let platform = match operating_system {
        "linux" => "linux",
        "macos" => "darwin",
        "freebsd" => "freebsd",
        _ => return None,
    };
    let architecture = match (architecture, platform) {
        ("x86_64", "linux") => "x86_64",
        ("aarch64", "linux") => "aarch64",
        ("i686", "linux") => "i686",
        ("arm", "linux") => "armv7l",
        ("riscv64", "linux") | ("riscv64gc", "linux") => "riscv64",
        ("powerpc64", "linux") => "powerpc64",
        ("powerpc64le", "linux") => "powerpc64le",
        ("s390x", "linux") => "s390x",
        ("x86_64", "darwin") => "x86_64",
        ("aarch64", "darwin") => "aarch64",
        ("x86_64", "freebsd") => "x86_64",
        ("aarch64", "freebsd") => "aarch64",
        _ => return None,
    };
    Some(format!("{architecture}-{platform}"))
}

#[cfg(test)]
fn nix_string_literal(value: &str) -> String {
    let mut literal = String::from("\"");
    let mut characters = value.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\\' => literal.push_str("\\\\"),
            '\"' => literal.push_str("\\\""),
            '$' if characters.peek() == Some(&'{') => literal.push_str("\\$"),
            '\n' => literal.push_str("\\n"),
            '\r' => literal.push_str("\\r"),
            '\t' => literal.push_str("\\t"),
            value => literal.push(value),
        }
    }
    literal.push('"');
    literal
}

fn open_private_log(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(file)
}

fn join_stdout(
    reader: Option<thread::JoinHandle<(Vec<u8>, io::Result<usize>)>>,
) -> Option<Vec<u8>> {
    reader.and_then(|reader| reader.join().ok().map(|(bytes, _)| bytes))
}

/// Continuously drain a Nix progress pipe into the retained log and parser.
/// The first read, write, or flush failure is returned so retention failures
/// become operational errors; parser errors stay best-effort inside the
/// collector and cannot change the returned result.
fn drain_progress_stderr<R: Read, W: Write>(
    mut pipe: R,
    mut log: W,
    collector: &NixProgressCollector,
) -> io::Result<()> {
    let mut buffer = [0u8; 8192];
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                log.write_all(&buffer[..count])?;
                collector.consume(&buffer[..count]);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    log.flush()
}

fn join_progress_stderr(reader: Option<thread::JoinHandle<io::Result<()>>>) -> io::Result<()> {
    match reader {
        None => Ok(()),
        Some(reader) => reader.join().unwrap_or_else(|_| {
            Err(io::Error::other(
                "Nix stderr reader terminated unexpectedly",
            ))
        }),
    }
}

fn nix_start_error(operation: &str, error: io::Error) -> String {
    if error.kind() == io::ErrorKind::NotFound {
        "Nix execution is selected but the 'nix' CLI is unavailable".to_owned()
    } else {
        format!("unable to start Nix for {operation}: {error}")
    }
}

fn nix_start_error_message(error: io::Error) -> String {
    if error.kind() == io::ErrorKind::NotFound {
        "Nix execution is selected but the 'nix' CLI is unavailable".to_owned()
    } else {
        format!("unable to start Nix task: {error}")
    }
}

fn status_text(status: ExitStatus) -> String {
    status
        .code()
        .map(|code| format!("exit code {code}"))
        .unwrap_or_else(|| "termination by signal".to_owned())
}

fn bounded_process_error(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let safe = super::details::normalize_display_text(&text);
    let first_line = safe
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    super::model::truncate_utf8(first_line.trim(), 512).0
}

#[cfg(test)]
mod tests {
    use super::{
        NixBackend, NixCli, NixTaskResult, PlannedSystem, batch_build_args, discover_catalog,
        drain_progress_stderr, format_args, host_nix_system, nix_log_store_path,
        nix_string_literal, parse_nix_id, plan_args, target_nix_system, validity_args,
    };
    use crate::check_command::interrupt::{CancellationToken, InterruptFlag};
    use crate::check_command::model::valid_system_name;
    use crate::check_command::nix_progress::NixProgressCollector;
    use crate::check_command::progress::ProgressSink;
    use crate::check_command::test_support::{fixture, nix_is_available};
    use std::ffi::OsString;
    use std::fs;
    use std::io::{self, Read, Write};
    use std::path::Path;

    struct CatalogStub {
        names: Vec<String>,
    }

    impl NixBackend for CatalogStub {
        fn host_system(&self, _root: &Path) -> Result<String, String> {
            Ok("x86_64-linux".to_owned())
        }

        fn discover(&self, _root: &Path, _system: &str) -> Result<Vec<String>, String> {
            Ok(self.names.clone())
        }

        fn format_workspace(
            &self,
            _root: &Path,
            _log_path: &Path,
            _cancellation: CancellationToken,
        ) -> NixTaskResult<()> {
            unreachable!("catalog test does not format the workspace")
        }

        fn plan_checks(&self, _root: &Path, _system: &str) -> Result<PlannedSystem, String> {
            unreachable!("catalog test does not plan checks")
        }

        fn realize_batch(
            &self,
            _root: &Path,
            _plans: &[super::CheckPlan],
            _fail_fast: bool,
            _log_path: &Path,
            _cancellation: CancellationToken,
            _progress: &crate::check_command::progress::ProgressSink,
        ) -> super::BatchRealization {
            unreachable!("catalog test does not realize checks")
        }

        fn validate_outputs(
            &self,
            _root: &Path,
            _outputs: &[String],
        ) -> Result<std::collections::BTreeSet<String>, String> {
            unreachable!("catalog test does not validate outputs")
        }

        fn read_derivation_log(&self, _root: &Path, _store_path: &str) -> Result<Vec<u8>, String> {
            unreachable!("catalog test does not retrieve derivation logs")
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-NIX-006"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-NIX-015"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-019"))]
    fn catalog_discovery_includes_every_attribute_except_legacy_recursive_output() {
        let backend = CatalogStub {
            names: vec![
                "unit-tests".to_owned(),
                "formatter".to_owned(),
                "bloomery:check".to_owned(),
            ],
        };
        let (catalog, notices) =
            discover_catalog(Path::new("."), &["x86_64-linux".to_owned()], &backend)
                .expect("catalog");

        assert!(catalog.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(catalog.contains(&"nix:x86_64-linux:unit-tests".to_owned()));
        assert!(catalog.contains(&"nix:x86_64-linux:formatter".to_owned()));
        assert!(!catalog.contains(&"nix:x86_64-linux:bloomery:check".to_owned()));
        assert_eq!(notices.len(), 1);
        assert!(notices[0].message.contains("bloomery:check"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-NIX-009"))]
    fn nix_ids_preserve_attribute_names_after_the_system_separator() {
        assert_eq!(
            parse_nix_id("nix:x86_64-linux:checks:with:colons"),
            Some(("x86_64-linux", "checks:with:colons"))
        );
        assert_eq!(parse_nix_id("nix:x86_64-linux:"), None);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-NIX-011"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-NIX-013"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-NIX-014"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-NIX-016"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-NIX-017"))]
    fn nix_invocations_are_pure_lock_safe_and_pass_attribute_names_as_literals() {
        let plan = strings(&plan_args("x86_64-linux"));
        let derivations = vec![
            "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-first.drv".to_owned(),
            "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-second.drv".to_owned(),
        ];
        let build = strings(&batch_build_args(&derivations, false));
        let fail_fast_build = strings(&batch_build_args(&derivations, true));
        let validity = strings(&validity_args());
        let format = strings(&format_args());
        for args in [&plan, &build, &format] {
            assert!(args.iter().any(|arg| arg == "--no-write-lock-file"));
            assert!(args.iter().any(|arg| arg == "--no-update-lock-file"));
            assert!(!args.iter().any(|arg| arg == "--impure"));
            assert!(!args.iter().any(|arg| arg.contains("nix-fast-build")));
        }
        assert!(plan.iter().any(|arg| arg.contains("x86_64-linux")));
        assert!(build.iter().any(|arg| arg == "--no-link"));
        assert!(build.iter().any(|arg| arg == "--keep-going"));
        assert!(!fail_fast_build.iter().any(|arg| arg == "--keep-going"));
        assert!(derivations.iter().all(|derivation| {
            build
                .iter()
                .any(|argument| *argument == format!("{derivation}^*"))
        }));
        assert_eq!(validity.first().map(String::as_str), Some("path-info"));
        assert_eq!(format.first().map(String::as_str), Some("fmt"));
        assert!(valid_system_name("aarch64-linux"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-NIX-016"))]
    fn real_nix_parses_unicode_control_and_quoted_check_attribute_names() {
        if !nix_is_available() {
            eprintln!("skipping real-Nix attribute parser test: Nix store is unavailable");
            return;
        }

        let (root, cache) = fixture(true);
        let system = host_nix_system().expect("host Nix system");
        let attributes = vec![
            "quoted attribute name with spaces".to_owned(),
            "Unicode ☃".to_owned(),
            "line\nbreak".to_owned(),
            "tab\tcharacter".to_owned(),
            format!("control-{}-character", char::from(1)),
            "percent-%0A-and-fragment-#".to_owned(),
        ];
        let names_json = serde_json::to_string(&attributes).expect("JSON attribute names");
        let names_expression = nix_string_literal(&names_json);
        let flake = format!(
            r#"{{
  description = "bloomery unusual attribute fixture";
  outputs = {{ self }}:
    let
      names = builtins.fromJSON {names_expression};
      shared = builtins.derivation {{
        name = "bloomery-unusual-attribute-check";
        system = "{system}";
        builder = "/bin/sh";
        args = [ "-c" "echo passed > $out" ];
      }};
    in {{
      checks."{system}" = builtins.listToAttrs (
        map (name: {{ inherit name; value = shared; }}) names
      );
    }};
}}"#
        );
        fs::write(root.join("flake.nix"), flake).expect("write unusual attribute fixture");

        let backend = NixCli::default();
        let discovered = backend
            .discover(&root, &system)
            .expect("discover unusual Nix attributes");
        let mut sorted_attributes = attributes.clone();
        sorted_attributes.sort();
        assert_eq!(discovered, sorted_attributes);

        let planned = backend
            .plan_checks(&root, &system)
            .expect("plan unusual attributes");
        let mut planned_attributes = planned
            .plans
            .iter()
            .map(|plan| plan.attribute.clone())
            .collect::<Vec<_>>();
        planned_attributes.sort();
        assert_eq!(planned_attributes, sorted_attributes);
        assert!(
            planned
                .plans
                .iter()
                .all(|plan| plan.derivation.is_some() && !plan.outputs.is_empty()),
            "every unusual attribute must evaluate to a derivation with outputs"
        );
        let (event_sender, _event_receiver) = std::sync::mpsc::channel();
        let progress = super::super::progress::ProgressSink::new(event_sender, false);
        let batch = backend.realize_batch(
            &root,
            &planned.plans,
            false,
            &root.join("unusual-batch.log"),
            CancellationToken::new(InterruptFlag::for_test()),
            &progress,
        );
        assert!(
            batch.failures.is_empty(),
            "unexpected realization failures: {:?}",
            batch.failures
        );
        assert!(batch.operational_error.is_none());
        let outputs = planned
            .plans
            .iter()
            .flat_map(|plan| plan.outputs.iter().cloned())
            .collect::<std::collections::BTreeSet<_>>();
        let valid = backend
            .validate_outputs(&root, &outputs.iter().cloned().collect::<Vec<_>>())
            .expect("output validity");
        assert_eq!(valid, outputs);
        assert!(!root.join("flake.lock").exists());

        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(cache);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-NIX-012"))]
    fn a_missing_nix_executable_is_an_operational_error() {
        let missing = NixCli::with_executable("/missing/bloomery-nix-cli");
        let error = missing
            .discover(Path::new("."), "x86_64-linux")
            .expect_err("missing executable");
        assert!(error.contains("nix"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-NIX-012"))]
    fn an_unavailable_nix_cli_fails_the_formatter_instead_of_skipping_it() {
        let missing = NixCli::with_executable("/missing/bloomery-nix-cli");
        let log = std::env::temp_dir().join("bloomery-missing-nix-format.log");
        let result = missing.format_workspace(
            Path::new("."),
            &log,
            CancellationToken::new(InterruptFlag::for_test()),
        );
        assert!(matches!(result, NixTaskResult::OperationalError(_)));
        let _ = fs::remove_file(log);
    }

    #[test]
    fn retained_stderr_write_failures_are_reported() {
        struct FailingWriter;
        impl Write for FailingWriter {
            fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("log full"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let collector = disabled_collector();
        let mut pipe = io::Cursor::new(b"@nix garbage\n".to_vec());
        assert!(drain_progress_stderr(&mut pipe, FailingWriter, &collector).is_err());
    }

    #[test]
    fn retained_stderr_read_failures_are_reported() {
        struct FailingReader;
        impl Read for FailingReader {
            fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("pipe broken"))
            }
        }

        let collector = disabled_collector();
        assert!(drain_progress_stderr(FailingReader, Vec::new(), &collector).is_err());
    }

    fn disabled_collector() -> NixProgressCollector {
        let (sender, _receiver) = std::sync::mpsc::channel();
        NixProgressCollector::new(ProgressSink::new(sender, false))
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-DETAIL-024"))]
    fn nix_log_paths_are_parsed_from_failure_hints_and_store_paths_are_validated() {
        let path = "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-failed-check.drv";
        let output = format!("error: check failed\nFor full logs, run 'nix log {path}'.\n");

        assert_eq!(nix_log_store_path(output.as_bytes()).as_deref(), Some(path));
        let multiline = format!("For full logs, run:\n  nix log {path}\n");
        assert_eq!(
            nix_log_store_path(multiline.as_bytes()).as_deref(),
            Some(path)
        );
        assert!(nix_log_store_path(b"For full logs, run: nix log /tmp/not-a-store.drv").is_none());
        assert!(
            nix_log_store_path(format!("build output mentions nix log {path}").as_bytes())
                .is_none()
        );
        assert!(
            nix_log_store_path(format!("build output mentions nix log {path}").as_bytes())
                .is_none()
        );
    }

    #[test]
    fn host_nix_system_uses_platform_mapping() {
        assert_eq!(
            target_nix_system("x86_64", "linux"),
            Some("x86_64-linux".to_owned())
        );
        assert_eq!(
            target_nix_system("aarch64", "macos"),
            Some("aarch64-darwin".to_owned())
        );
        assert!(target_nix_system("mips", "linux").is_none());
        assert!(valid_system_name(&host_nix_system().expect("host system")));
    }

    fn strings(arguments: &[OsString]) -> Vec<String> {
        arguments
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect()
    }
}
