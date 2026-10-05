use super::model::{
    CheckRecord, FailureLocation, FailureRecord, Notice, Outcome, RunRecord, RunSelection,
    RunStatus,
};
use serde_json::{Map, Value, json};
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static RUN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct RunStore {
    root: PathBuf,
    namespace: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RunAllocation {
    pub id: String,
    pub directory: PathBuf,
    logs: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMetadata {
    pub revision: Option<String>,
    pub dirty: Option<bool>,
}

impl RunStore {
    pub fn open(root: &Path, cache_base: Option<&Path>) -> Result<Self, String> {
        let root = fs::canonicalize(root)
            .map_err(|error| format!("unable to resolve workspace root: {error}"))?;
        let cache_base = match cache_base {
            Some(path) => path.to_path_buf(),
            None => user_cache_directory()?,
        };
        let namespace = cache_base
            .join("bloomery")
            .join("check")
            .join(workspace_key(&root));
        create_private_dir(&namespace).map_err(|error| {
            format!(
                "unable to create private check cache {}: {error}",
                namespace.display()
            )
        })?;
        create_private_dir(&namespace.join("runs")).map_err(|error| {
            format!(
                "unable to create run cache {}: {error}",
                namespace.display()
            )
        })?;
        Ok(Self { root, namespace })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn allocate(&self) -> Result<RunAllocation, String> {
        let runs = self.namespace.join("runs");
        for _ in 0..100 {
            let id = new_run_id();
            let directory = runs.join(&id);
            match fs::create_dir(&directory) {
                Ok(()) => {
                    set_private_directory_permissions(&directory).map_err(|error| {
                        format!(
                            "unable to secure run directory {}: {error}",
                            directory.display()
                        )
                    })?;
                    let logs = directory.join("logs");
                    create_private_dir(&logs).map_err(|error| {
                        format!(
                            "unable to create run log directory {}: {error}",
                            logs.display()
                        )
                    })?;
                    return Ok(RunAllocation {
                        id,
                        directory,
                        logs,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!("unable to allocate a check run: {error}"));
                }
            }
        }
        Err("unable to allocate a unique check run ID".to_owned())
    }

    pub fn finalize(&self, allocation: &RunAllocation, record: &RunRecord) -> Result<(), String> {
        if allocation.id != record.id || record.root != self.root.to_string_lossy() {
            return Err("run manifest identity does not match its cache allocation".to_owned());
        }
        let manifest = allocation.directory.join("manifest.json");
        if manifest.exists() {
            return Err("completed run manifests are immutable".to_owned());
        }

        let temporary = allocation.directory.join("manifest.json.tmp");
        let bytes = serde_json::to_vec(&record_to_json(record))
            .map_err(|error| format!("unable to serialize run manifest: {error}"))?;
        write_private_file(&temporary, &bytes)
            .map_err(|error| format!("unable to write run manifest: {error}"))?;
        fs::rename(&temporary, &manifest)
            .map_err(|error| format!("unable to publish completed run manifest: {error}"))?;
        File::open(&allocation.directory)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("unable to sync completed run manifest: {error}"))?;
        Ok(())
    }

    pub fn load(&self, id: &str) -> Result<(RunRecord, PathBuf), String> {
        if !valid_run_id(id) {
            return Err(format!("invalid retained run ID '{id}'"));
        }
        let directory = self.namespace.join("runs").join(id);
        let manifest = directory.join("manifest.json");
        let record = read_manifest(&manifest)?;
        self.validate_record(&record, id)?;
        Ok((record, directory))
    }

    pub fn latest(&self) -> Result<(RunRecord, PathBuf), String> {
        let runs = self.namespace.join("runs");
        let entries = fs::read_dir(&runs)
            .map_err(|error| format!("unable to read retained check runs: {error}"))?;
        let mut completed = Vec::new();
        for entry in entries {
            let entry =
                entry.map_err(|error| format!("unable to read retained run entry: {error}"))?;
            if !entry
                .file_type()
                .map_err(|error| format!("unable to inspect retained run entry: {error}"))?
                .is_dir()
            {
                continue;
            }
            let directory = entry.path();
            let manifest = directory.join("manifest.json");
            if !manifest.exists() {
                continue;
            }
            let id = entry.file_name().to_string_lossy().into_owned();
            let record = read_manifest(&manifest)?;
            self.validate_record(&record, &id)?;
            completed.push((record, directory));
        }
        completed.sort_by(|(left, _), (right, _)| {
            left.completed_at
                .cmp(&right.completed_at)
                .then(left.id.cmp(&right.id))
        });
        completed
            .pop()
            .ok_or_else(|| "no completed check run is retained for this workspace".to_owned())
    }

    fn validate_record(&self, record: &RunRecord, expected_id: &str) -> Result<(), String> {
        if record.id != expected_id {
            return Err("retained run manifest ID does not match its directory".to_owned());
        }
        if record.root != self.root.to_string_lossy() {
            return Err("retained run belongs to a different workspace".to_owned());
        }
        Ok(())
    }
}

impl RunAllocation {
    pub fn prepare_log(&self, name: &str) -> Result<PathBuf, String> {
        if !valid_log_name(name) {
            return Err("invalid retained log name".to_owned());
        }
        let path = self.logs.join(name);
        let mut options = OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options
            .open(&path)
            .map_err(|error| format!("unable to create task log: {error}"))?;
        set_private_file_permissions(&path)
            .map_err(|error| format!("unable to secure task log {}: {error}", path.display()))?;
        drop(file);
        Ok(path)
    }
}

/// Freeze a derivation's normalized records on first retrieval. The OS lock is
/// shared by concurrent processes and released automatically if one exits.
/// Aliased failures reference one snapshot without modifying the run manifest.
pub(super) fn retain_derivation_records(
    run_directory: &Path,
    store_path: &str,
    resolve: impl FnOnce() -> Vec<String>,
) -> Result<Vec<String>, String> {
    let directory = run_directory.join("derivation-logs");
    create_private_dir(&directory)
        .map_err(|error| format!("unable to create derivation log cache: {error}"))?;
    let key = cache_key(store_path.as_bytes());
    let lock_path = directory.join(format!("{key}.lock"));
    let mut options = OpenOptions::new();
    options.create(true).truncate(false).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options
        .open(&lock_path)
        .map_err(|error| format!("unable to open derivation log cache lock: {error}"))?;
    set_private_file_permissions(&lock_path)
        .map_err(|error| format!("unable to secure derivation log cache lock: {error}"))?;
    lock.lock()
        .map_err(|error| format!("unable to lock derivation log cache: {error}"))?;

    let snapshot = directory.join(format!("{key}.json"));
    match fs::read(&snapshot) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|error| format!("retained derivation log snapshot is invalid: {error}")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let records = resolve();
            let bytes = serde_json::to_vec(&records)
                .map_err(|error| format!("unable to serialize derivation log snapshot: {error}"))?;
            let temporary = directory.join(format!("{key}.json.tmp"));
            write_private_file(&temporary, &bytes)
                .map_err(|error| format!("unable to retain derivation log snapshot: {error}"))?;
            fs::rename(&temporary, &snapshot)
                .map_err(|error| format!("unable to publish derivation log snapshot: {error}"))?;
            File::open(&directory)
                .and_then(|file| file.sync_all())
                .map_err(|error| format!("unable to sync derivation log snapshot: {error}"))?;
            Ok(records)
        }
        Err(error) => Err(format!("unable to read derivation log snapshot: {error}")),
    }
}

pub fn source_metadata(root: &Path) -> SourceMetadata {
    source_metadata_from_git(root).unwrap_or(SourceMetadata {
        revision: None,
        dirty: None,
    })
}

fn source_metadata_from_git(root: &Path) -> Option<SourceMetadata> {
    source_metadata_from_git_with(root, OsStr::new("git"))
}

fn source_metadata_from_git_with(root: &Path, executable: &OsStr) -> Option<SourceMetadata> {
    let revision = Command::new(executable)
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|revision| !revision.is_empty())?;
    let dirty = Command::new(executable)
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| !output.stdout.is_empty());
    Some(SourceMetadata {
        revision: Some(revision),
        dirty,
    })
}

pub fn resolve_log_path(run_directory: &Path, reference: &str) -> Result<PathBuf, String> {
    let name = reference
        .strip_prefix("logs/")
        .ok_or_else(|| "retained log reference is invalid".to_owned())?;
    if !valid_log_name(name) {
        return Err("retained log reference is invalid".to_owned());
    }
    Ok(run_directory.join("logs").join(name))
}

pub fn user_cache_directory() -> Result<PathBuf, String> {
    if let Some(value) = std::env::var_os("XDG_CACHE_HOME") {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            return Ok(path);
        }
    }

    #[cfg(target_os = "macos")]
    let suffix = "Library/Caches";
    #[cfg(not(target_os = "macos"))]
    let suffix = ".cache";

    if let Some(home) = std::env::var_os("HOME") {
        return Ok(PathBuf::from(home).join(suffix));
    }
    #[cfg(windows)]
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        return Ok(PathBuf::from(local_app_data));
    }
    Err("unable to determine the user's cache directory".to_owned())
}

fn new_run_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("r{:x}-{:x}-{:x}", std::process::id(), timestamp, sequence)
}

fn valid_run_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn valid_log_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        && !name.starts_with('.')
}

fn create_private_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    set_private_directory_permissions(path)
}

fn set_private_directory_permissions(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn set_private_file_permissions(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn write_private_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    set_private_file_permissions(path)?;
    file.write_all(contents)?;
    file.sync_all()
}

fn workspace_key(root: &Path) -> String {
    cache_key(root.to_string_lossy().as_bytes())
}

fn cache_key(bytes: &[u8]) -> String {
    let first = stable_hash(bytes, 0xcbf29ce484222325);
    let second = stable_hash(bytes, 0x84222325cbf29ce4);
    format!("{first:016x}{second:016x}")
}

fn stable_hash(bytes: &[u8], seed: u64) -> u64 {
    bytes.iter().fold(seed, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn record_to_json(record: &RunRecord) -> Value {
    let outcomes = record
        .outcomes
        .iter()
        .map(|outcome| {
            let mut value = json!({
                "check": outcome.id,
                "outcome": outcome.outcome.as_str(),
                "logs": outcome.logs,
            });
            if let Some(cause) = &outcome.blocked_by {
                value["blocked_by"] = Value::String(cause.clone());
            }
            value
        })
        .collect::<Vec<_>>();
    let failures = record
        .failures
        .iter()
        .map(|failure| {
            let mut value = json!({
                "id": failure.id,
                "check": failure.check,
                "code": failure.code,
                "message": failure.message,
                "notes": failure.notes,
                "focus_tail": failure.focus_tail,
            });
            if let Some(subject) = &failure.subject {
                value["subject"] = Value::String(subject.clone());
            }
            if let Some(location) = &failure.location {
                value["location"] = json!({
                    "path": location.path,
                    "line": location.line,
                    "column": location.column,
                });
            }
            if let Some(log) = &failure.log {
                value["log"] = Value::String(log.clone());
            }
            if let Some(nix_log) = &failure.nix_log {
                value["nix_log"] = Value::String(nix_log.clone());
            }
            value
        })
        .collect::<Vec<_>>();
    let notices = record
        .notices
        .iter()
        .map(|notice| json!({"code": notice.code, "message": notice.message}))
        .collect::<Vec<_>>();

    json!({
        "version": 1,
        "id": record.id,
        "root": record.root,
        "started_at": record.started_at,
        "completed_at": record.completed_at,
        "source": {
            "revision": record.source_revision,
            "dirty": record.source_dirty,
        },
        "status": record.status.as_str(),
        "selection": {
            "selectors": record.selection.selectors,
            "systems": record.selection.systems,
            "selected_checks": record.selection.selected_checks,
            "partial": record.selection.partial,
        },
        "outcomes": outcomes,
        "failures": failures,
        "notices": notices,
    })
}

fn read_manifest(path: &Path) -> Result<RunRecord, String> {
    let bytes = fs::read(path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            format!("retained check run is unavailable: {}", path.display())
        } else {
            format!("unable to read retained check run: {error}")
        }
    })?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("retained check manifest is invalid: {error}"))?;
    record_from_json(&value)
}

fn record_from_json(value: &Value) -> Result<RunRecord, String> {
    if number(value, "version")? != 1 {
        return Err("unsupported retained check manifest version".to_owned());
    }
    let selection = value
        .get("selection")
        .and_then(Value::as_object)
        .ok_or_else(|| "retained check manifest has no selection".to_owned())?;
    let source = match value.get("source") {
        None | Some(Value::Null) => None,
        Some(Value::Object(source)) => Some(source),
        _ => return Err("retained check manifest has invalid source metadata".to_owned()),
    };
    let outcomes = value
        .get("outcomes")
        .and_then(Value::as_array)
        .ok_or_else(|| "retained check manifest has no outcomes".to_owned())?
        .iter()
        .map(|entry| {
            let outcome = Outcome::parse(required_string(entry, "outcome")?.as_str())
                .ok_or_else(|| "retained check manifest has an invalid outcome".to_owned())?;
            Ok(CheckRecord {
                id: required_string(entry, "check")?,
                outcome,
                blocked_by: optional_string(entry, "blocked_by")?,
                logs: string_array(entry, "logs")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let failures = value
        .get("failures")
        .and_then(Value::as_array)
        .ok_or_else(|| "retained check manifest has no failures".to_owned())?
        .iter()
        .map(|entry| {
            let location = match entry.get("location") {
                Some(Value::Object(location)) => Some(FailureLocation {
                    path: required_string(entry.get("location").unwrap(), "path")?,
                    line: optional_number(location.get("line"))?,
                    column: optional_number(location.get("column"))?,
                }),
                Some(Value::Null) | None => None,
                _ => return Err("retained failure has an invalid location".to_owned()),
            };
            Ok(FailureRecord {
                id: required_string(entry, "id")?,
                check: required_string(entry, "check")?,
                code: required_string(entry, "code")?,
                subject: optional_string(entry, "subject")?,
                location,
                message: required_string(entry, "message")?,
                notes: string_array(entry, "notes")?,
                log: optional_string(entry, "log")?,
                nix_log: optional_string(entry, "nix_log")?,
                focus_tail: entry
                    .get("focus_tail")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let notices = value
        .get("notices")
        .and_then(Value::as_array)
        .ok_or_else(|| "retained check manifest has no notices".to_owned())?
        .iter()
        .map(|entry| {
            Ok(Notice {
                code: required_string(entry, "code")?,
                message: required_string(entry, "message")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(RunRecord {
        id: required_string(value, "id")?,
        root: required_string(value, "root")?,
        started_at: number(value, "started_at")?,
        completed_at: number(value, "completed_at")?,
        source_revision: source
            .map(|source| optional_string_object(source, "revision"))
            .transpose()?
            .flatten(),
        source_dirty: source
            .map(|source| optional_bool_object(source, "dirty"))
            .transpose()?
            .flatten(),
        status: RunStatus::parse(required_string(value, "status")?.as_str())
            .ok_or_else(|| "retained check manifest has an invalid status".to_owned())?,
        selection: RunSelection {
            selectors: string_array_object(selection, "selectors")?,
            systems: string_array_object(selection, "systems")?,
            selected_checks: string_array_object(selection, "selected_checks")?,
            partial: selection
                .get("partial")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
        outcomes,
        failures,
        notices,
    })
}

fn required_string(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("retained check manifest field '{key}' is missing or invalid"))
}

fn optional_string_object(value: &Map<String, Value>, key: &str) -> Result<Option<String>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        _ => Err(format!("retained source metadata field '{key}' is invalid")),
    }
}

fn optional_bool_object(value: &Map<String, Value>, key: &str) -> Result<Option<bool>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        _ => Err(format!("retained source metadata field '{key}' is invalid")),
    }
}

fn optional_string(value: &Value, key: &str) -> Result<Option<String>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        _ => Err(format!("retained check manifest field '{key}' is invalid")),
    }
}

fn number(value: &Value, key: &str) -> Result<u64, String> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("retained check manifest field '{key}' is missing or invalid"))
}

fn optional_number(value: Option<&Value>) -> Result<Option<usize>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|number| usize::try_from(number).ok())
            .map(Some)
            .ok_or_else(|| "retained check location has an invalid number".to_owned()),
    }
}

fn string_array(value: &Value, key: &str) -> Result<Vec<String>, String> {
    let array = value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("retained check manifest field '{key}' is missing or invalid"))?;
    array
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("retained check manifest field '{key}' is invalid"))
        })
        .collect()
}

fn string_array_object(value: &Map<String, Value>, key: &str) -> Result<Vec<String>, String> {
    let array = value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("retained check selection field '{key}' is missing or invalid"))?;
    array
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("retained check selection field '{key}' is invalid"))
        })
        .collect()
}

pub(super) fn timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::{RunStore, source_metadata_from_git_with, timestamp_millis};
    use crate::check_command::model::{
        CheckRecord, FailureRecord, Outcome, RunRecord, RunSelection, RunStatus,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE: AtomicU64 = AtomicU64::new(0);

    fn paths() -> (PathBuf, PathBuf) {
        let id = FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "bloomery-check-store-root-{}-{id}",
            std::process::id()
        ));
        let cache = std::env::temp_dir().join(format!(
            "bloomery-check-store-cache-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("workspace root");
        (root, cache)
    }

    fn sample_record(store: &RunStore, id: String) -> RunRecord {
        RunRecord {
            id,
            root: store.root().to_string_lossy().into_owned(),
            started_at: timestamp_millis(),
            completed_at: timestamp_millis(),
            source_revision: None,
            source_dirty: None,
            status: RunStatus::Failed,
            selection: RunSelection {
                selectors: Vec::new(),
                systems: Vec::new(),
                selected_checks: vec!["static:structure".to_owned()],
                partial: false,
            },
            outcomes: vec![CheckRecord {
                id: "static:structure".to_owned(),
                outcome: Outcome::Failed,
                blocked_by: None,
                logs: Vec::new(),
            }],
            failures: vec![FailureRecord {
                id: "f1".to_owned(),
                check: "static:structure".to_owned(),
                code: "MissingDocument".to_owned(),
                subject: Some("REQ-001".to_owned()),
                location: None,
                message: "full diagnostic message".to_owned(),
                notes: vec!["complete note".to_owned()],
                log: None,
                nix_log: None,
                focus_tail: false,
            }],
            notices: Vec::new(),
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-DETAIL-001"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-DETAIL-002"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-DETAIL-003"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-DETAIL-021"))]
    fn cache_runs_are_isolated_unique_and_atomically_finalized() {
        let (root, cache) = paths();
        let store = RunStore::open(&root, Some(&cache)).expect("run store");
        let first = store.allocate().expect("first run");
        let second = store.allocate().expect("second run");
        assert_ne!(first.id, second.id);
        assert!(first.directory.starts_with(&cache));

        let mut record = sample_record(&store, first.id.clone());
        store.finalize(&first, &record).expect("finalize run");
        assert!(first.directory.join("manifest.json").is_file());
        assert!(!first.directory.join("manifest.json.tmp").exists());
        assert!(store.finalize(&first, &record).is_err());

        record.id = second.id.clone();
        store
            .finalize(&second, &record)
            .expect("finalize second run");
        assert_eq!(store.load(&first.id).expect("load first").0.id, first.id);
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(cache);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-DETAIL-004"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-DETAIL-005"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-DETAIL-014"))]
    fn retrieval_selects_explicit_or_latest_completed_runs_without_fallback() {
        let (root, cache) = paths();
        let store = RunStore::open(&root, Some(&cache)).expect("run store");
        let first = store.allocate().expect("first run");
        let mut old = sample_record(&store, first.id.clone());
        old.completed_at = 10;
        store.finalize(&first, &old).expect("finalize first");
        let second = store.allocate().expect("second run");
        let mut recent = sample_record(&store, second.id.clone());
        recent.completed_at = 20;
        store.finalize(&second, &recent).expect("finalize second");

        assert_eq!(store.latest().expect("latest run").0.id, second.id);
        assert_eq!(store.load(&first.id).expect("explicit run").0.id, first.id);
        assert!(store.load("missing-run").is_err());
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(cache);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-DETAIL-022"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-DETAIL-023"))]
    fn available_git_revision_and_dirty_state_are_retained_in_runs() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let (root, cache) = paths();
            let fake_git = root.join("fake-git");
            fs::write(
                &fake_git,
                r#"#!/bin/sh
if [ "$1" = "rev-parse" ]; then
  printf 'revision-fixture'
elif [ "$1" = "status" ]; then
  if [ -f dirty-marker ]; then
    printf 'dirty'
  fi
fi
"#,
            )
            .expect("write fake Git command");
            fs::set_permissions(&fake_git, fs::Permissions::from_mode(0o700))
                .expect("make fake Git executable");

            let clean = source_metadata_from_git_with(&root, fake_git.as_os_str())
                .expect("read available Git metadata");
            assert_eq!(clean.revision.as_deref(), Some("revision-fixture"));
            assert_eq!(clean.dirty, Some(false));

            fs::write(root.join("dirty-marker"), "untracked source change\n")
                .expect("create a dirty worktree marker");
            let dirty = source_metadata_from_git_with(&root, fake_git.as_os_str())
                .expect("read dirty Git metadata");
            assert_eq!(dirty.revision, clean.revision);
            assert_eq!(dirty.dirty, Some(true));

            let store = RunStore::open(&root, Some(&cache)).expect("run store");
            let clean_allocation = store.allocate().expect("clean run allocation");
            let mut clean_record = sample_record(&store, clean_allocation.id.clone());
            clean_record.source_revision = clean.revision.clone();
            clean_record.source_dirty = clean.dirty;
            store
                .finalize(&clean_allocation, &clean_record)
                .expect("finalize clean run");

            let retained_clean = store
                .load(&clean_allocation.id)
                .expect("reload clean run")
                .0;
            assert_eq!(
                retained_clean.source_revision.as_deref(),
                Some("revision-fixture")
            );
            assert_eq!(retained_clean.source_dirty, Some(false));

            let dirty_allocation = store.allocate().expect("dirty run allocation");
            let mut dirty_record = sample_record(&store, dirty_allocation.id.clone());
            dirty_record.source_revision = dirty.revision.clone();
            dirty_record.source_dirty = dirty.dirty;
            store
                .finalize(&dirty_allocation, &dirty_record)
                .expect("finalize dirty run");

            let retained_dirty = store
                .load(&dirty_allocation.id)
                .expect("reload dirty run")
                .0;
            assert_eq!(
                retained_dirty.source_revision.as_deref(),
                Some("revision-fixture")
            );
            assert_eq!(retained_dirty.source_dirty, Some(true));

            let _ = fs::remove_dir_all(root);
            let _ = fs::remove_dir_all(cache);
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-DETAIL-019"))]
    fn cache_directories_files_and_logs_use_private_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let (root, cache) = paths();
        let store = RunStore::open(&root, Some(&cache)).expect("run store");
        let allocation = store.allocate().expect("run allocation");
        let log = allocation.prepare_log("task-0.log").expect("private log");
        let file_mode = fs::metadata(log)
            .expect("log metadata")
            .permissions()
            .mode()
            & 0o777;
        let directory_mode = fs::metadata(&allocation.directory)
            .expect("directory metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600);
        assert_eq!(directory_mode, 0o700);
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(cache);
    }
}
