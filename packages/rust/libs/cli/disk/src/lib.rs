//! Read-only measurement of the Nix store space used by a Bloomery workspace
//! tree or a single addressed derivation.

use bloomery_cli_types::DiskArgs;
pub use bloomery_cli_types::DiskScope;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Nix expression that maps a package output set to named measurement targets.
const PACKAGES_APPLY: &str = "packages: builtins.map (name: { category = \"runtime\"; name = name; path = packages.${name}.outPath; }) (builtins.attrNames packages)";

/// Nix expression that maps a check output set to named measurement targets,
/// dropping the legacy recursive `bloomery:check` attribute.
const CHECKS_APPLY: &str = "checks: builtins.map (name: { category = \"build\"; name = name; path = checks.${name}.outPath; }) (builtins.attrNames (builtins.removeAttrs checks [\"bloomery:check\"]))";

/// Extension of a Nix derivation store path.
const DERIVATION_SUFFIX: &str = ".drv";

/// One named output path selected for measurement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskTarget {
    pub category: String,
    pub name: String,
    pub path: String,
}

/// One store path in a target's transitive closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathSize {
    pub path: String,
    pub nar_size: u64,
}

/// The realized closure of one measured target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetClosure {
    pub target: DiskTarget,
    pub realized: bool,
    pub entries: Vec<PathSize>,
}

/// One derivation inside a category, with the bytes it exclusively claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskEntry {
    pub name: String,
    pub bytes: u64,
    pub unmeasured: bool,
}

/// A named group of measured derivations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskCategory {
    pub name: String,
    pub bytes: u64,
    pub entries: Vec<DiskEntry>,
}

/// Result of measuring the full tree or one addressed derivation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiskReport {
    pub bytes: u64,
    pub systems: Vec<String>,
    pub derivation: Option<String>,
    pub unmeasured: Vec<String>,
    pub categories: Vec<DiskCategory>,
}

/// Failure class for a disk measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskErrorKind {
    Usage,
    Operational,
}

/// A disk measurement that could not produce a total.
#[derive(Debug, Clone)]
pub struct DiskError {
    kind: DiskErrorKind,
    message: String,
}

impl DiskError {
    fn usage(message: impl Into<String>) -> Self {
        Self {
            kind: DiskErrorKind::Usage,
            message: message.into(),
        }
    }

    fn operational(message: impl Into<String>) -> Self {
        Self {
            kind: DiskErrorKind::Operational,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> DiskErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn exit_code(&self) -> u8 {
        2
    }
}

impl fmt::Display for DiskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DiskError {}

/// The Nix operations that disk measurement needs, injectable for testing.
pub trait NixQuery {
    fn is_available(&self) -> bool;
    fn host_system(&self, root: &Path) -> Result<String, String>;
    fn resolve_tree(
        &self,
        root: &Path,
        systems: &[String],
        scope: DiskScope,
    ) -> Result<Vec<DiskTarget>, String>;
    fn resolve_derivation(&self, root: &Path, address: &str) -> Result<Vec<DiskTarget>, String>;
    fn closure_sizes(
        &self,
        root: &Path,
        targets: &[DiskTarget],
    ) -> Result<Vec<TargetClosure>, String>;
}

/// Measure the full tree or one addressed derivation using `backend`.
pub fn run(root: &Path, args: &DiskArgs, backend: &impl NixQuery) -> Result<DiskReport, DiskError> {
    if !backend.is_available() {
        return Err(DiskError::operational(
            "required executable 'nix' was not found on PATH",
        ));
    }

    let ResolvedTargets {
        mut targets,
        systems,
        derivation,
    } = resolve_targets(root, args, backend)?;
    targets.sort_by(target_order);
    targets.dedup();
    let closures = backend
        .closure_sizes(root, &targets)
        .map_err(|error| DiskError::operational(format!("unable to query store sizes: {error}")))?;

    let mut claimed: BTreeSet<String> = BTreeSet::new();
    let mut categories: Vec<DiskCategory> = Vec::new();
    let mut unmeasured: Vec<String> = Vec::new();
    let mut total: u64 = 0;
    for closure in &closures {
        let mut entry_bytes: u64 = 0;
        for entry in &closure.entries {
            if claimed.insert(entry.path.clone()) {
                entry_bytes += entry.nar_size;
                total += entry.nar_size;
            }
        }
        if !closure.realized {
            unmeasured.push(closure.target.path.clone());
        }
        let category = category_mut(&mut categories, &closure.target.category);
        category.bytes += entry_bytes;
        match category
            .entries
            .iter_mut()
            .find(|entry| entry.name == closure.target.name)
        {
            Some(entry) => {
                entry.bytes += entry_bytes;
                entry.unmeasured |= !closure.realized;
            }
            None => category.entries.push(DiskEntry {
                name: closure.target.name.clone(),
                bytes: entry_bytes,
                unmeasured: !closure.realized,
            }),
        }
    }

    Ok(DiskReport {
        bytes: total,
        systems,
        derivation,
        unmeasured,
        categories,
    })
}

/// Exit code for a completed measurement: `0` when every target was realized
/// and `1` when at least one target was unmeasured.
pub fn exit_code(report: &DiskReport) -> u8 {
    if report.unmeasured.is_empty() { 0 } else { 1 }
}

struct ResolvedTargets {
    targets: Vec<DiskTarget>,
    systems: Vec<String>,
    derivation: Option<String>,
}

fn resolve_targets(
    root: &Path,
    args: &DiskArgs,
    backend: &impl NixQuery,
) -> Result<ResolvedTargets, DiskError> {
    if let Some(address) = &args.derivation {
        let mut targets = backend.resolve_derivation(root, address).map_err(|error| {
            DiskError::usage(format!("unable to resolve derivation '{address}': {error}"))
        })?;
        targets.sort_by(target_order);
        targets.dedup();
        if targets.is_empty() {
            return Err(DiskError::usage(format!(
                "derivation '{address}' resolved to no store paths"
            )));
        }
        return Ok(ResolvedTargets {
            targets,
            systems: Vec::new(),
            derivation: Some(address.clone()),
        });
    }

    let systems = if args.systems.is_empty() {
        vec![backend.host_system(root).map_err(|error| {
            DiskError::operational(format!("unable to determine the host Nix system: {error}"))
        })?]
    } else {
        deduplicate(&args.systems)
    };
    let mut targets = backend
        .resolve_tree(root, &systems, args.scope)
        .map_err(|error| DiskError::operational(format!("unable to evaluate the tree: {error}")))?;
    targets.sort_by(target_order);
    targets.dedup();
    Ok(ResolvedTargets {
        targets,
        systems,
        derivation: None,
    })
}

fn category_mut<'a>(categories: &'a mut Vec<DiskCategory>, name: &str) -> &'a mut DiskCategory {
    if let Some(index) = categories.iter().position(|category| category.name == name) {
        return &mut categories[index];
    }
    categories.push(DiskCategory {
        name: name.to_owned(),
        bytes: 0,
        entries: Vec::new(),
    });
    categories.last_mut().expect("category was just pushed")
}

fn target_order(left: &DiskTarget, right: &DiskTarget) -> std::cmp::Ordering {
    category_rank(&left.category)
        .cmp(&category_rank(&right.category))
        .then_with(|| left.name.cmp(&right.name))
        .then_with(|| left.path.cmp(&right.path))
}

fn category_rank(category: &str) -> u8 {
    match category {
        "runtime" => 0,
        "build" => 1,
        "derivation" => 2,
        _ => 3,
    }
}

fn deduplicate(values: &[String]) -> Vec<String> {
    let mut deduplicated = Vec::new();
    for value in values {
        if !deduplicated.contains(value) {
            deduplicated.push(value.clone());
        }
    }
    deduplicated
}

/// Render a concise size summary for a terminal.
pub fn render_summary(report: &DiskReport) -> String {
    let mut output = format!(
        "Disk use: {} ({} bytes)\n",
        format_bytes(report.bytes),
        report.bytes
    );
    if let Some(derivation) = &report.derivation {
        output.push_str(&format!("Derivation: {derivation}\n"));
    }
    if !report.systems.is_empty() {
        output.push_str(&format!("Systems: {}\n", report.systems.join(", ")));
    }
    for category in &report.categories {
        if category.name == "derivation" {
            continue;
        }
        output.push_str(&format!(
            "{}: {}\n",
            category_label(&category.name),
            format_bytes(category.bytes)
        ));
    }
    if report.unmeasured.is_empty() {
        output.push_str("Unmeasured derivations: none\n");
    } else {
        output.push_str(&format!(
            "Unmeasured derivations: {}\n",
            report.unmeasured.len()
        ));
    }
    output
}

fn category_label(category: &str) -> &str {
    match category {
        "runtime" => "Runtime",
        "build" => "Build",
        other => other,
    }
}

/// Render a report for a terminal as an indented category tree.
pub fn render_text(report: &DiskReport) -> String {
    let mut output = format!(
        "Disk use: {} ({} bytes)\n",
        format_bytes(report.bytes),
        report.bytes
    );
    if let Some(derivation) = &report.derivation {
        output.push_str(&format!("Derivation: {derivation}\n"));
    }
    if !report.systems.is_empty() {
        output.push_str(&format!("Systems: {}\n", report.systems.join(", ")));
    }
    for (index, category) in report.categories.iter().enumerate() {
        let last_category = index + 1 == report.categories.len();
        let branch = if last_category {
            "└── "
        } else {
            "├── "
        };
        output.push_str(&format!(
            "{branch}{} ({})\n",
            category.name,
            format_bytes(category.bytes)
        ));
        let child_prefix = if last_category { "    " } else { "│   " };
        for (entry_index, entry) in category.entries.iter().enumerate() {
            let last_entry = entry_index + 1 == category.entries.len();
            let entry_branch = if last_entry {
                "└── "
            } else {
                "├── "
            };
            let size = if entry.unmeasured {
                format!("{}, unmeasured", format_bytes(entry.bytes))
            } else {
                format_bytes(entry.bytes)
            };
            output.push_str(&format!(
                "{child_prefix}{entry_branch}{} ({size})\n",
                entry.name
            ));
        }
    }
    output
}

/// Format a byte count with binary units.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// Whether an address names a Nix store path rather than a flake output.
pub fn is_store_path(address: &str) -> bool {
    address.starts_with("/nix/store/")
}

/// Real Nix CLI implementation of [`NixQuery`].
#[derive(Debug, Clone)]
pub struct NixCli {
    executable: OsString,
    store_executable: OsString,
}

impl Default for NixCli {
    fn default() -> Self {
        Self {
            executable: OsString::from("nix"),
            store_executable: OsString::from("nix-store"),
        }
    }
}

impl NixQuery for NixCli {
    fn is_available(&self) -> bool {
        Command::new(&self.executable)
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    fn host_system(&self, _root: &Path) -> Result<String, String> {
        host_nix_system()
    }

    fn resolve_tree(
        &self,
        root: &Path,
        systems: &[String],
        scope: DiskScope,
    ) -> Result<Vec<DiskTarget>, String> {
        let mut targets = Vec::new();
        for system in systems {
            if matches!(scope, DiskScope::All | DiskScope::Runtime) {
                targets.extend(self.eval_targets(
                    root,
                    &format!("packages.{system}"),
                    PACKAGES_APPLY,
                )?);
            }
            if matches!(scope, DiskScope::All | DiskScope::Build) {
                targets.extend(self.eval_targets(
                    root,
                    &format!("checks.{system}"),
                    CHECKS_APPLY,
                )?);
            }
        }
        Ok(targets)
    }

    fn resolve_derivation(&self, root: &Path, address: &str) -> Result<Vec<DiskTarget>, String> {
        if is_store_path(address) {
            if address.ends_with(DERIVATION_SUFFIX) {
                return Ok(self
                    .derivation_outputs(root, address)?
                    .into_iter()
                    .map(|path| DiskTarget {
                        category: "derivation".to_owned(),
                        name: address.to_owned(),
                        path,
                    })
                    .collect());
            }
            return Ok(vec![DiskTarget {
                category: "derivation".to_owned(),
                name: address.to_owned(),
                path: address.to_owned(),
            }]);
        }
        let arguments = vec![
            OsString::from("eval"),
            OsString::from("--raw"),
            OsString::from("--no-write-lock-file"),
            OsString::from("--no-update-lock-file"),
            OsString::from(format!(".#{address}")),
            OsString::from("--apply"),
            OsString::from("value: value.outPath"),
        ];
        let output = run_tool(&self.executable, &arguments, root)?;
        if !output.status.success() {
            return Err(nix_stderr(&output));
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if path.is_empty() {
            return Err(format!("'{address}' evaluated to no output path"));
        }
        Ok(vec![DiskTarget {
            category: "derivation".to_owned(),
            name: address.to_owned(),
            path,
        }])
    }

    fn closure_sizes(
        &self,
        root: &Path,
        targets: &[DiskTarget],
    ) -> Result<Vec<TargetClosure>, String> {
        let mut closures = Vec::with_capacity(targets.len());
        for target in targets {
            let arguments = vec![
                OsString::from("path-info"),
                OsString::from("--json"),
                OsString::from("--recursive"),
                OsString::from(&target.path),
            ];
            let output = run_tool(&self.executable, &arguments, root)?;
            if !output.status.success() {
                closures.push(TargetClosure {
                    target: target.clone(),
                    realized: false,
                    entries: Vec::new(),
                });
                continue;
            }
            let value: serde_json::Value = serde_json::from_slice(&output.stdout)
                .map_err(|error| format!("Nix returned invalid store info JSON: {error}"))?;
            let realized = match &value {
                serde_json::Value::Object(map) => {
                    map.get(&target.path).is_some_and(|entry| !entry.is_null())
                }
                serde_json::Value::Array(items) => !items.is_empty(),
                _ => false,
            };
            if !realized {
                closures.push(TargetClosure {
                    target: target.clone(),
                    realized: false,
                    entries: Vec::new(),
                });
                continue;
            }
            closures.push(TargetClosure {
                target: target.clone(),
                realized: true,
                entries: parse_path_info(&value)?,
            });
        }
        Ok(closures)
    }
}

impl NixCli {
    fn eval_targets(
        &self,
        root: &Path,
        attribute: &str,
        apply: &str,
    ) -> Result<Vec<DiskTarget>, String> {
        let arguments = vec![
            OsString::from("eval"),
            OsString::from("--no-write-lock-file"),
            OsString::from("--no-update-lock-file"),
            OsString::from("--json"),
            OsString::from(format!(".#{attribute}")),
            OsString::from("--apply"),
            OsString::from(apply),
        ];
        let output = run_tool(&self.executable, &arguments, root)?;
        if !output.status.success() {
            return Err(nix_stderr(&output));
        }
        serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("Nix returned invalid output path JSON: {error}"))
    }

    fn derivation_outputs(&self, root: &Path, derivation: &str) -> Result<Vec<String>, String> {
        let arguments = vec![
            OsString::from("--query"),
            OsString::from("--outputs"),
            OsString::from(derivation),
        ];
        let output = run_tool(&self.store_executable, &arguments, root)?;
        if !output.status.success() {
            return Err(nix_stderr(&output));
        }
        let outputs = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if outputs.is_empty() {
            return Err(format!("derivation '{derivation}' has no outputs"));
        }
        Ok(outputs)
    }
}

fn run_tool(executable: &OsStr, arguments: &[OsString], root: &Path) -> Result<Output, String> {
    Command::new(executable)
        .args(arguments)
        .current_dir(root)
        .output()
        .map_err(|error| format!("unable to invoke {}: {error}", executable.to_string_lossy()))
}

fn nix_stderr(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if stderr.is_empty() {
        format!("Nix exited with {}", output.status)
    } else {
        stderr
    }
}

/// Parse `nix path-info --json` output in either its keyed-object or legacy
/// array shape.
fn parse_path_info(value: &serde_json::Value) -> Result<Vec<PathSize>, String> {
    match value {
        serde_json::Value::Object(map) => map
            .iter()
            .filter(|(_, info)| !info.is_null())
            .map(|(path, info)| {
                let nar_size = info
                    .get("narSize")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| format!("store path '{path}' has no narSize"))?;
                Ok(PathSize {
                    path: path.clone(),
                    nar_size,
                })
            })
            .collect(),
        serde_json::Value::Array(items) => items
            .iter()
            .map(|item| {
                let path = item
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| "store path entry has no path".to_owned())?;
                let nar_size = item
                    .get("narSize")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| format!("store path '{path}' has no narSize"))?;
                Ok(PathSize {
                    path: path.to_owned(),
                    nar_size,
                })
            })
            .collect(),
        _ => Err("Nix store info is neither an object nor an array".to_owned()),
    }
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

fn valid_system_name(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

fn configured_nix_system() -> Option<String> {
    let mut configuration = std::collections::BTreeMap::new();
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
        .filter_map(|path| std::fs::read_to_string(path).ok())
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
mod tests {
    use super::{
        CHECKS_APPLY, DiskArgs, DiskErrorKind, DiskReport, DiskScope, DiskTarget, NixQuery,
        TargetClosure, exit_code, format_bytes, is_store_path, render_summary, render_text, run,
    };
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::path::Path;

    #[derive(Default)]
    struct FakeNix {
        available: bool,
        host: String,
        tree: Vec<DiskTarget>,
        derivation: Option<Vec<DiskTarget>>,
        closures: BTreeMap<String, TargetClosure>,
        tree_calls: RefCell<Vec<Vec<String>>>,
        scope_calls: RefCell<Vec<DiskScope>>,
        derivation_calls: RefCell<Vec<String>>,
    }

    impl FakeNix {
        fn realized(target: DiskTarget, entries: &[(&str, u64)]) -> TargetClosure {
            TargetClosure {
                target,
                realized: true,
                entries: entries
                    .iter()
                    .map(|(path, nar_size)| super::PathSize {
                        path: (*path).to_owned(),
                        nar_size: *nar_size,
                    })
                    .collect(),
            }
        }

        fn unmeasured(target: DiskTarget) -> TargetClosure {
            TargetClosure {
                target,
                realized: false,
                entries: Vec::new(),
            }
        }
    }

    fn target(category: &str, name: &str, path: &str) -> DiskTarget {
        DiskTarget {
            category: category.to_owned(),
            name: name.to_owned(),
            path: path.to_owned(),
        }
    }

    impl NixQuery for FakeNix {
        fn is_available(&self) -> bool {
            self.available
        }

        fn host_system(&self, _root: &Path) -> Result<String, String> {
            Ok(self.host.clone())
        }

        fn resolve_tree(
            &self,
            _root: &Path,
            systems: &[String],
            scope: DiskScope,
        ) -> Result<Vec<DiskTarget>, String> {
            self.tree_calls.borrow_mut().push(systems.to_vec());
            self.scope_calls.borrow_mut().push(scope);
            Ok(self
                .tree
                .iter()
                .filter(|target| match scope {
                    DiskScope::All => true,
                    DiskScope::Runtime => target.category == "runtime",
                    DiskScope::Build => target.category == "build",
                })
                .cloned()
                .collect())
        }

        fn resolve_derivation(
            &self,
            _root: &Path,
            address: &str,
        ) -> Result<Vec<DiskTarget>, String> {
            self.derivation_calls.borrow_mut().push(address.to_owned());
            self.derivation
                .clone()
                .ok_or_else(|| format!("unknown derivation '{address}'"))
        }

        fn closure_sizes(
            &self,
            _root: &Path,
            targets: &[DiskTarget],
        ) -> Result<Vec<TargetClosure>, String> {
            Ok(targets
                .iter()
                .map(|target| {
                    self.closures
                        .get(&target.path)
                        .cloned()
                        .unwrap_or_else(|| FakeNix::unmeasured(target.clone()))
                })
                .collect())
        }
    }

    fn full_tree_fake() -> FakeNix {
        let app = target("runtime", "default", "/nix/store/aaa-app");
        let check = target("build", "app:test", "/nix/store/bbb-check");
        let mut closures = BTreeMap::new();
        closures.insert(
            "/nix/store/aaa-app".to_owned(),
            FakeNix::realized(app.clone(), &[("/nix/store/aaa-app", 100)]),
        );
        closures.insert(
            "/nix/store/bbb-check".to_owned(),
            FakeNix::realized(check.clone(), &[("/nix/store/bbb-check", 200)]),
        );
        FakeNix {
            available: true,
            host: "x86_64-linux".to_owned(),
            tree: vec![app, check],
            closures,
            ..FakeNix::default()
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-DISK-COMMAND-001"))]
    #[cfg_attr(any(), bloomery("CLI-DISK-COMMAND-002"))]
    #[cfg_attr(any(), bloomery("CLI-DISK-MEASUREMENT-003"))]
    #[cfg_attr(any(), bloomery("CLI-DISK-MEASUREMENT-005"))]
    fn disk_measures_full_tree_without_realizing() {
        let fake = full_tree_fake();
        let report = run(Path::new("/repo"), &DiskArgs::default(), &fake).expect("measurement");

        assert_eq!(report.bytes, 300);
        assert_eq!(report.systems, vec!["x86_64-linux".to_owned()]);
        assert_eq!(report.derivation, None);
        assert!(report.unmeasured.is_empty());
        assert_eq!(exit_code(&report), 0);
        assert_eq!(
            fake.tree_calls.borrow().as_slice(),
            &[vec!["x86_64-linux".to_owned()]]
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-DISK-COMMAND-003"))]
    fn disk_measures_only_the_addressed_derivation() {
        let mut fake = full_tree_fake();
        let app = target(
            "derivation",
            "packages.x86_64-linux.default",
            "/nix/store/aaa-app",
        );
        fake.derivation = Some(vec![app.clone()]);
        fake.closures.insert(
            "/nix/store/aaa-app".to_owned(),
            FakeNix::realized(app, &[("/nix/store/aaa-app", 100)]),
        );
        let args = DiskArgs {
            tree: false,
            scope: DiskScope::All,
            derivation: Some("packages.x86_64-linux.default".to_owned()),
            systems: Vec::new(),
        };
        let report = run(Path::new("/repo"), &args, &fake).expect("measurement");

        assert_eq!(report.bytes, 100);
        assert!(report.systems.is_empty());
        assert_eq!(
            report.derivation.as_deref(),
            Some("packages.x86_64-linux.default")
        );
        assert!(fake.tree_calls.borrow().is_empty());
        assert_eq!(
            fake.derivation_calls.borrow().as_slice(),
            &["packages.x86_64-linux.default".to_owned()]
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-DISK-COMMAND-004"))]
    #[cfg_attr(any(), bloomery("CLI-DISK-COMMAND-005"))]
    fn disk_accepts_flake_output_and_store_paths() {
        assert!(!is_store_path("packages.x86_64-linux.default"));
        assert!(!is_store_path("checks.x86_64-linux.my-crate:test"));
        assert!(is_store_path(
            "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-app"
        ));
        assert!(is_store_path(
            "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-app.drv"
        ));

        let mut fake = full_tree_fake();
        let app = target("derivation", "default", "/nix/store/aaa-app");
        fake.derivation = Some(vec![app.clone()]);
        fake.closures.insert(
            "/nix/store/aaa-app".to_owned(),
            FakeNix::realized(app, &[("/nix/store/aaa-app", 100)]),
        );
        for address in [
            "packages.x86_64-linux.default",
            "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-app",
        ] {
            let args = DiskArgs {
                tree: false,
                scope: DiskScope::All,
                derivation: Some(address.to_owned()),
                systems: Vec::new(),
            };
            run(Path::new("/repo"), &args, &fake).expect("addressed measurement");
        }
        assert_eq!(
            fake.derivation_calls.borrow().as_slice(),
            &[
                "packages.x86_64-linux.default".to_owned(),
                "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-app".to_owned(),
            ]
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-DISK-COMMAND-006"))]
    fn disk_defaults_to_the_host_system() {
        let mut fake = full_tree_fake();
        fake.host = "aarch64-darwin".to_owned();
        let report = run(Path::new("/repo"), &DiskArgs::default(), &fake).expect("measurement");

        assert_eq!(report.systems, vec!["aarch64-darwin".to_owned()]);
        assert_eq!(
            fake.tree_calls.borrow().as_slice(),
            &[vec!["aarch64-darwin".to_owned()]]
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-DISK-COMMAND-007"))]
    fn disk_deduplicates_explicit_systems() {
        let fake = full_tree_fake();
        let args = DiskArgs {
            tree: false,
            scope: DiskScope::All,
            derivation: None,
            systems: vec!["x86_64-linux".to_owned(), "x86_64-linux".to_owned()],
        };
        let report = run(Path::new("/repo"), &args, &fake).expect("measurement");

        assert_eq!(report.systems, vec!["x86_64-linux".to_owned()]);
        assert_eq!(
            fake.tree_calls.borrow().as_slice(),
            &[vec!["x86_64-linux".to_owned()]]
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-DISK-COMMAND-010"))]
    fn disk_scope_selects_runtime_or_build() {
        let fake = full_tree_fake();

        let runtime = DiskArgs {
            tree: false,
            scope: DiskScope::Runtime,
            derivation: None,
            systems: Vec::new(),
        };
        let report = run(Path::new("/repo"), &runtime, &fake).expect("runtime measurement");
        assert_eq!(report.bytes, 100);
        assert_eq!(report.categories.len(), 1);
        assert_eq!(report.categories[0].name, "runtime");
        assert_eq!(fake.scope_calls.borrow().as_slice(), &[DiskScope::Runtime]);

        let build = DiskArgs {
            tree: false,
            scope: DiskScope::Build,
            derivation: None,
            systems: Vec::new(),
        };
        let report = run(Path::new("/repo"), &build, &fake).expect("build measurement");
        assert_eq!(report.bytes, 200);
        assert_eq!(report.categories.len(), 1);
        assert_eq!(report.categories[0].name, "build");
        assert_eq!(
            fake.scope_calls.borrow().as_slice(),
            &[DiskScope::Runtime, DiskScope::Build]
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-DISK-COMMAND-009"))]
    fn disk_rejects_unresolvable_derivations() {
        let mut fake = full_tree_fake();
        fake.derivation = None;
        let args = DiskArgs {
            tree: false,
            scope: DiskScope::All,
            derivation: Some("packages.x86_64-linux.missing".to_owned()),
            systems: Vec::new(),
        };
        let error = run(Path::new("/repo"), &args, &fake).expect_err("unresolvable derivation");

        assert_eq!(error.kind(), DiskErrorKind::Usage);
        assert_eq!(error.exit_code(), 2);
        assert!(error.message().contains("packages.x86_64-linux.missing"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-DISK-MEASUREMENT-001"))]
    #[cfg_attr(any(), bloomery("CLI-DISK-MEASUREMENT-002"))]
    #[cfg_attr(any(), bloomery("CLI-DISK-MEASUREMENT-011"))]
    fn disk_sums_unique_closure_nar_sizes() {
        let mut fake = full_tree_fake();
        let app = target("runtime", "default", "/nix/store/aaa-app");
        let check = target("build", "app:test", "/nix/store/bbb-check");
        fake.closures.insert(
            "/nix/store/aaa-app".to_owned(),
            FakeNix::realized(
                app,
                &[("/nix/store/aaa-app", 100), ("/nix/store/shared-lib", 50)],
            ),
        );
        fake.closures.insert(
            "/nix/store/bbb-check".to_owned(),
            FakeNix::realized(
                check,
                &[("/nix/store/bbb-check", 200), ("/nix/store/shared-lib", 50)],
            ),
        );
        let report = run(Path::new("/repo"), &DiskArgs::default(), &fake).expect("measurement");

        assert_eq!(report.bytes, 350);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-DISK-MEASUREMENT-004"))]
    fn disk_excludes_the_recursive_check_attribute() {
        assert!(CHECKS_APPLY.contains("removeAttrs"));
        assert!(CHECKS_APPLY.contains("bloomery:check"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-DISK-MEASUREMENT-006"))]
    #[cfg_attr(any(), bloomery("CLI-DISK-MEASUREMENT-007"))]
    fn disk_reports_unrealized_derivations_and_fails_nonzero() {
        let mut fake = full_tree_fake();
        fake.closures.insert(
            "/nix/store/bbb-check".to_owned(),
            FakeNix::unmeasured(target("build", "app:test", "/nix/store/bbb-check")),
        );
        let report = run(Path::new("/repo"), &DiskArgs::default(), &fake).expect("measurement");

        assert_eq!(report.bytes, 100);
        assert_eq!(report.unmeasured, vec!["/nix/store/bbb-check".to_owned()]);
        assert_eq!(exit_code(&report), 1);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-DISK-MEASUREMENT-008"))]
    fn disk_reports_unavailable_nix() {
        let fake = FakeNix {
            available: false,
            ..FakeNix::default()
        };
        let error = run(Path::new("/repo"), &DiskArgs::default(), &fake).expect_err("no Nix");

        assert_eq!(error.kind(), DiskErrorKind::Operational);
        assert_eq!(error.exit_code(), 2);
        assert!(error.message().contains("nix"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-DISK-MEASUREMENT-009"))]
    #[cfg_attr(any(), bloomery("CLI-DISK-MEASUREMENT-010"))]
    fn disk_renders_binary_units_and_integer_json_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MiB");

        let report = DiskReport {
            bytes: 1536,
            systems: vec!["x86_64-linux".to_owned()],
            derivation: None,
            unmeasured: Vec::new(),
            categories: Vec::new(),
        };
        let text = render_text(&report);
        assert!(text.contains("Disk use: 1.5 KiB (1536 bytes)"));
        assert!(text.contains("Systems: x86_64-linux"));

        let value = serde_json::to_value(&report).expect("JSON");
        assert!(value["bytes"].is_u64());
        assert_eq!(value["bytes"], 1536);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-DISK-REPORT-001"))]
    #[cfg_attr(any(), bloomery("CLI-DISK-REPORT-002"))]
    #[cfg_attr(any(), bloomery("CLI-DISK-REPORT-003"))]
    fn disk_categories_partition_the_total() {
        let mut fake = full_tree_fake();
        let app = target("runtime", "default", "/nix/store/aaa-app");
        let check = target("build", "app:test", "/nix/store/bbb-check");
        fake.closures.insert(
            "/nix/store/aaa-app".to_owned(),
            FakeNix::realized(
                app,
                &[("/nix/store/aaa-app", 100), ("/nix/store/shared-lib", 50)],
            ),
        );
        fake.closures.insert(
            "/nix/store/bbb-check".to_owned(),
            FakeNix::realized(
                check,
                &[("/nix/store/bbb-check", 200), ("/nix/store/shared-lib", 50)],
            ),
        );
        let report = run(Path::new("/repo"), &DiskArgs::default(), &fake).expect("measurement");

        assert_eq!(report.bytes, 350);
        let category_total: u64 = report
            .categories
            .iter()
            .map(|category| category.bytes)
            .sum();
        assert_eq!(category_total, report.bytes);
        let packages = &report.categories[0];
        assert_eq!(packages.name, "runtime");
        assert_eq!(packages.entries[0].name, "default");
        assert_eq!(packages.entries[0].bytes, 150);
        let checks = &report.categories[1];
        assert_eq!(checks.name, "build");
        assert_eq!(checks.entries[0].bytes, 200);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-DISK-REPORT-006"))]
    #[cfg_attr(any(), bloomery("CLI-DISK-REPORT-007"))]
    fn disk_renders_a_size_summary_without_the_tree() {
        let report = DiskReport {
            bytes: 1536,
            systems: vec!["x86_64-linux".to_owned()],
            derivation: None,
            unmeasured: vec!["/nix/store/aaa-check".to_owned()],
            categories: vec![super::DiskCategory {
                name: "runtime".to_owned(),
                bytes: 1536,
                entries: vec![super::DiskEntry {
                    name: "default".to_owned(),
                    bytes: 1536,
                    unmeasured: false,
                }],
            }],
        };
        let text = render_summary(&report);
        assert!(text.contains("Disk use: 1.5 KiB (1536 bytes)"));
        assert!(text.contains("Systems: x86_64-linux"));
        assert!(text.contains("Runtime: 1.5 KiB"));
        assert!(text.contains("Unmeasured derivations: 1"));
        assert!(!text.contains("├──"));
        assert!(!text.contains("default ("));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-DISK-REPORT-004"))]
    fn disk_renders_an_indented_category_tree() {
        let report = DiskReport {
            bytes: 300,
            systems: vec!["x86_64-linux".to_owned()],
            derivation: None,
            unmeasured: Vec::new(),
            categories: vec![
                super::DiskCategory {
                    name: "runtime".to_owned(),
                    bytes: 100,
                    entries: vec![super::DiskEntry {
                        name: "default".to_owned(),
                        bytes: 100,
                        unmeasured: false,
                    }],
                },
                super::DiskCategory {
                    name: "build".to_owned(),
                    bytes: 200,
                    entries: vec![super::DiskEntry {
                        name: "app:test".to_owned(),
                        bytes: 0,
                        unmeasured: true,
                    }],
                },
            ],
        };
        let text = render_text(&report);
        assert!(text.contains("├── runtime (100 B)"));
        assert!(text.contains("│   └── default (100 B)"));
        assert!(text.contains("└── build (200 B)"));
        assert!(text.contains("    └── app:test (0 B, unmeasured)"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-DISK-REPORT-005"))]
    fn disk_orders_categories_and_entries_deterministically() {
        let mut fake = full_tree_fake();
        let checks_z = target("build", "z:test", "/nix/store/zzz-check");
        let checks_a = target("build", "a:test", "/nix/store/aaa-check");
        fake.tree = vec![checks_z.clone(), checks_a.clone()];
        fake.closures.clear();
        fake.closures.insert(
            "/nix/store/zzz-check".to_owned(),
            FakeNix::realized(checks_z, &[("/nix/store/zzz-check", 1)]),
        );
        fake.closures.insert(
            "/nix/store/aaa-check".to_owned(),
            FakeNix::realized(checks_a, &[("/nix/store/aaa-check", 2)]),
        );
        let report = run(Path::new("/repo"), &DiskArgs::default(), &fake).expect("measurement");

        let names: Vec<&str> = report
            .categories
            .iter()
            .flat_map(|category| category.entries.iter().map(|entry| entry.name.as_str()))
            .collect();
        assert_eq!(names, vec!["a:test", "z:test"]);
    }
}
