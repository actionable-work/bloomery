//! Nix CLI backend for the build command.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Stdio};

/// One discovered production package and its evaluated derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePlan {
    pub name: String,
    pub system: String,
    pub derivation: Option<String>,
    pub eval_error: Option<String>,
}

/// Result of one batched realization.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Realization {
    pub failed: BTreeMap<String, String>,
    pub canceled: BTreeSet<String>,
    pub not_run: BTreeSet<String>,
    pub operational_error: Option<String>,
}

/// Backend operations required by the build command.
pub trait NixBackend {
    fn host_system(&self, root: &Path) -> Result<String, String>;
    fn plan_packages(&self, root: &Path, system: &str) -> Result<Vec<PackagePlan>, String>;
    fn realize(&self, root: &Path, derivations: &[String]) -> Realization;
    fn output_paths(
        &self,
        root: &Path,
        derivations: &[String],
    ) -> Result<BTreeMap<String, Vec<String>>, String>;
    fn validate_outputs(&self, root: &Path, outputs: &[String])
    -> Result<BTreeSet<String>, String>;
}

/// Nix CLI-backed implementation.
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

impl NixCli {
    fn run(&self, root: &Path, args: &[OsString]) -> Result<std::process::Output, String> {
        Command::new(&self.executable)
            .args(args)
            .current_dir(root)
            .stdin(Stdio::null())
            .output()
            .map_err(|error| format!("unable to run nix: {error}"))
    }
}

impl NixBackend for NixCli {
    fn host_system(&self, _root: &Path) -> Result<String, String> {
        bloomery_check_command::host_nix_system()
    }

    fn plan_packages(&self, root: &Path, system: &str) -> Result<Vec<PackagePlan>, String> {
        let args = plan_args(system);
        let output = self.run(root, &args)?;
        if !output.status.success() {
            return Err(format!(
                "unable to evaluate packages.{system}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        parse_packages_json(system, &output.stdout)
    }

    fn realize(&self, root: &Path, derivations: &[String]) -> Realization {
        if derivations.is_empty() {
            return Realization::default();
        }
        let args = realize_args(derivations);
        let output = match self.run(root, &args) {
            Ok(output) => output,
            Err(error) => {
                return Realization {
                    operational_error: Some(error),
                    ..Realization::default()
                };
            }
        };
        parse_build_result(
            derivations,
            output.status.success(),
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).as_ref(),
        )
    }

    fn output_paths(
        &self,
        root: &Path,
        derivations: &[String],
    ) -> Result<BTreeMap<String, Vec<String>>, String> {
        if derivations.is_empty() {
            return Ok(BTreeMap::new());
        }
        let args = derivation_show_args(derivations);
        let output = self.run(root, &args)?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        parse_derivation_show_json(&output.stdout)
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
        let output = self.run(root, &args)?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        parse_path_info_json(&output.stdout)
    }
}

pub(crate) fn parse_packages_json(system: &str, bytes: &[u8]) -> Result<Vec<PackagePlan>, String> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("Nix returned invalid packages JSON: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "Nix packages catalog is not an object".to_owned())?;
    let mut plans = Vec::with_capacity(object.len());
    for (name, entry) in object {
        let success = entry
            .get("success")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let derivation = entry
            .get("drvPath")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .filter(|path| valid_derivation_path(path));
        plans.push(PackagePlan {
            name: name.clone(),
            system: system.to_owned(),
            derivation: if success { derivation } else { None },
            eval_error: (!success).then(|| format!("{name} did not evaluate to a derivation")),
        });
    }
    plans.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(plans)
}

pub(crate) fn parse_build_result(
    derivations: &[String],
    success: bool,
    code: Option<i32>,
    stderr: &str,
) -> Realization {
    if code == Some(130) {
        return Realization {
            canceled: derivations.iter().cloned().collect(),
            ..Realization::default()
        };
    }
    if success {
        return Realization::default();
    }
    let mut failed = BTreeMap::new();
    for line in stderr.lines() {
        if let Some(derivation) = parse_cannot_build(line) {
            failed.insert(derivation, line.trim().to_owned());
        }
    }
    if failed.is_empty() {
        return Realization {
            operational_error: Some(format!(
                "nix build failed without a per-derivation failure: {}",
                stderr.trim()
            )),
            ..Realization::default()
        };
    }
    Realization {
        failed,
        ..Realization::default()
    }
}

pub(crate) fn parse_derivation_show_json(
    bytes: &[u8],
) -> Result<BTreeMap<String, Vec<String>>, String> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("Nix returned invalid derivation JSON: {error}"))?;
    let object = value
        .get("derivations")
        .and_then(serde_json::Value::as_object)
        .or_else(|| value.as_object())
        .ok_or_else(|| "Nix derivation show is not an object".to_owned())?;
    let mut paths = BTreeMap::new();
    for (derivation, entry) in object {
        let mut outputs = Vec::new();
        if let Some(output_map) = entry.get("outputs").and_then(serde_json::Value::as_object) {
            for output in output_map.values() {
                if let Some(path) = output.get("path").and_then(serde_json::Value::as_str) {
                    outputs.push(full_store_path(path));
                }
            }
        }
        paths.insert(full_store_path(derivation), outputs);
    }
    Ok(paths)
}

fn full_store_path(value: &str) -> String {
    if value.starts_with("/nix/store/") {
        value.to_owned()
    } else {
        format!("/nix/store/{value}")
    }
}

pub(crate) fn parse_path_info_json(bytes: &[u8]) -> Result<BTreeSet<String>, String> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("Nix returned invalid path-info JSON: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "Nix path-info is not an object".to_owned())?;
    Ok(object.keys().cloned().collect())
}

pub(crate) fn plan_args(system: &str) -> Vec<OsString> {
    let mut args = eval_base_args();
    args.extend([
        OsString::from("--json"),
        OsString::from(format!(".#packages.{system}")),
        OsString::from("--apply"),
        OsString::from(
            r#"packages:
  builtins.mapAttrs (name: value:
    let attempt = builtins.tryEval value.drvPath;
    in if attempt.success
       then { success = true; drvPath = attempt.value; }
       else { success = false; }
  ) packages"#,
        ),
    ]);
    args
}

pub(crate) fn realize_args(derivations: &[String]) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("build"),
        OsString::from("--no-link"),
        OsString::from("--keep-going"),
        OsString::from("--no-write-lock-file"),
        OsString::from("--no-update-lock-file"),
    ];
    args.extend(
        derivations
            .iter()
            .map(|derivation| OsString::from(format!("{derivation}^*"))),
    );
    args
}

fn derivation_show_args(derivations: &[String]) -> Vec<OsString> {
    let mut args = vec![OsString::from("derivation"), OsString::from("show")];
    args.extend(derivations.iter().map(OsString::from));
    args
}

fn validity_args() -> Vec<OsString> {
    vec![
        OsString::from("path-info"),
        OsString::from("--json"),
        OsString::from("--no-write-lock-file"),
        OsString::from("--no-update-lock-file"),
    ]
}

fn eval_base_args() -> Vec<OsString> {
    ["eval", "--no-write-lock-file", "--no-update-lock-file"]
        .into_iter()
        .map(OsString::from)
        .collect()
}

fn valid_derivation_path(path: &str) -> bool {
    let Some(name) = path.strip_prefix("/nix/store/") else {
        return false;
    };
    !name.is_empty()
        && name.ends_with(".drv")
        && !name.chars().any(|character| {
            character.is_control() || character.is_whitespace() || character == '/'
        })
}

fn parse_cannot_build(line: &str) -> Option<String> {
    let start = line.find("Cannot build '")? + "Cannot build '".len();
    let rest = &line[start..];
    let end = rest.find('\'')?;
    let derivation = &rest[..end];
    valid_derivation_path(derivation).then(|| derivation.to_owned())
}
