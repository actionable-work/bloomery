#![allow(clippy::result_large_err)]

mod lockfile;
mod metadata;
mod nix_migration;
mod recommendations;
mod runner;
mod selection;

pub use recommendations::RECOMMENDATION_CATALOG_VERSION;
pub use runner::{CommandOutput, CommandRunner, SystemCommandRunner};
pub use selection::{
    BARE_UPDATE_VALUE, Ecosystem, UpdatePlan, UpdateSelection, UsageError, parse_cli_update,
    parse_update_list,
};

use bloomery_model::config::{LoadedConfig, load_with_raw};
use lockfile::{PublicationOutcome, publish_candidate, serialize_candidate};
use recommendations::{missing_recommendations, recommendation_key};
use serde::Serialize;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::Write;
use std::path::Path;

#[derive(Debug)]
pub struct SyncError {
    stage: String,
    message: String,
    completed_stages: Vec<String>,
    may_be_partially_synchronized: bool,
}

impl SyncError {
    fn new(stage: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            stage: stage.into(),
            message: message.into(),
            completed_stages: Vec::new(),
            may_be_partially_synchronized: false,
        }
    }

    fn after_progress(
        stage: impl Into<String>,
        message: impl Into<String>,
        state: &RunState,
    ) -> Self {
        Self {
            stage: stage.into(),
            message: message.into(),
            completed_stages: state.completed.clone(),
            may_be_partially_synchronized: state.may_be_partially_synchronized,
        }
    }

    pub fn stage(&self) -> &str {
        &self.stage
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn completed_stages(&self) -> &[String] {
        &self.completed_stages
    }

    pub fn may_be_partially_synchronized(&self) -> bool {
        self.may_be_partially_synchronized
    }

    pub fn exit_code(&self) -> u8 {
        1
    }
}

impl fmt::Display for SyncError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "sync failed during {}: {}",
            self.stage, self.message
        )?;
        if !self.completed_stages.is_empty() {
            write!(
                formatter,
                "\ncompleted stages: {}",
                self.completed_stages.join(", ")
            )?;
        }
        if self.may_be_partially_synchronized {
            write!(
                formatter,
                "\nwarning: Cargo and Nix locks may be partially synchronized; after fixing the error, rerun `bloomery sync`."
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for SyncError {}

#[derive(Debug, Default)]
struct RunState {
    completed: Vec<String>,
    may_be_partially_synchronized: bool,
    tool_stderr: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SyncRecommendation {
    pub key: String,
    pub benefit: String,
    pub guidance: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SyncReport {
    pub reconciled_locks: Vec<String>,
    pub updated_ecosystems: Vec<String>,
    pub skipped_updates: Vec<String>,
    pub completed_stages: Vec<String>,
    pub warnings: Vec<String>,
    pub recommendations: Vec<SyncRecommendation>,
    pub tool_stderr: Vec<String>,
}

#[derive(Debug, Default)]
struct Advisories {
    warnings: Vec<String>,
    recommendations: Vec<SyncRecommendation>,
}

impl RunState {
    fn completed(&mut self, stage: &str) {
        self.completed.push(stage.to_owned());
    }

    fn failure(&self, stage: &str, message: impl Into<String>) -> SyncError {
        SyncError::after_progress(stage, message, self)
    }
}

/// Reconcile Cargo and Bloomery locks in `root`, optionally updating selected
/// ecosystems. The caller is responsible for parsing CLI syntax before calling
/// this function so usage errors cannot reach a mutating stage.
pub fn run(
    root: &Path,
    selection: &UpdateSelection,
    runner: &mut impl CommandRunner,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<SyncReport, SyncError> {
    let (plan, loaded_config) = preflight(root, selection, runner)?;
    let advisories = collect_advisories(&loaded_config);
    emit_advisories(&advisories, stderr)?;

    let mut state = RunState::default();
    if plan.update_nix {
        write_progress(
            stdout,
            "==> Updating Nix flake inputs...",
            "progress output",
            &state,
        )?;
        state.may_be_partially_synchronized = true;
        let output = execute(
            runner,
            OsStr::new("nix"),
            &[OsString::from("flake"), OsString::from("update")],
            root,
            "Nix input update",
            &state,
        )?;
        state.completed("Nix input update");
        record_tool_stderr(&mut state, &output.stderr);
        forward_stream(&output.stderr, stderr, "Nix input update stderr", &state)?;
        forward_stream(&output.stdout, stdout, "Nix input update output", &state)?;
    }

    let cargo_manifest = root.join("Cargo.toml");
    if plan.update_rust {
        write_progress(
            stdout,
            "==> Updating Rust dependencies...",
            "progress output",
            &state,
        )?;
        state.may_be_partially_synchronized = true;
        let arguments = [
            OsString::from("update"),
            OsString::from("--manifest-path"),
            cargo_manifest.as_os_str().to_owned(),
        ];
        let output = execute(
            runner,
            OsStr::new("cargo"),
            &arguments,
            root,
            "Rust dependency update",
            &state,
        )?;
        state.completed("Rust dependency update");
        record_tool_stderr(&mut state, &output.stderr);
        forward_stream(&output.stderr, stderr, "Cargo update stderr", &state)?;
        forward_stream(&output.stdout, stdout, "Cargo update output", &state)?;
    } else {
        write_progress(
            stdout,
            "==> Reconciling Cargo.lock with workspace manifests...",
            "progress output",
            &state,
        )?;
        state.may_be_partially_synchronized = true;
        let arguments = cargo_metadata_arguments(&cargo_manifest, false);
        let output = execute(
            runner,
            OsStr::new("cargo"),
            &arguments,
            root,
            "Cargo lock reconciliation",
            &state,
        )?;
        state.completed("Cargo lock reconciliation");
        record_tool_stderr(&mut state, &output.stderr);
        forward_stream(&output.stderr, stderr, "Cargo metadata stderr", &state)?;
    }

    write_progress(
        stdout,
        "==> Reading final locked Cargo metadata...",
        "progress output",
        &state,
    )?;
    let final_metadata_arguments = cargo_metadata_arguments(&cargo_manifest, true);
    let final_metadata = execute(
        runner,
        OsStr::new("cargo"),
        &final_metadata_arguments,
        root,
        "final Cargo metadata",
        &state,
    )?;
    state.completed("final locked Cargo metadata");
    record_tool_stderr(&mut state, &final_metadata.stderr);
    forward_stream(
        &final_metadata.stderr,
        stderr,
        "final Cargo metadata stderr",
        &state,
    )?;

    write_progress(
        stdout,
        "==> Generating and validating bloomery.lock...",
        "progress output",
        &state,
    )?;
    let cargo_lock_path = root.join("Cargo.lock");
    let cargo_lock = std::fs::read(&cargo_lock_path).map_err(|error| {
        state.failure(
            "Cargo.lock hashing",
            format!("unable to read '{}': {error}", cargo_lock_path.display()),
        )
    })?;
    let bloomery_lock = metadata::create_lock(&final_metadata.stdout, &cargo_lock)
        .map_err(|error| state.failure("Bloomery lock generation", error))?;
    let candidate = serialize_candidate(&bloomery_lock)
        .map_err(|error| state.failure("Bloomery lock validation", error))?;
    state.completed("Bloomery lock generation");

    let bloomery_lock_path = root.join("bloomery.lock");
    match publish_candidate(&bloomery_lock_path, &candidate)
        .map_err(|error| state.failure("Bloomery lock publication", error))?
    {
        PublicationOutcome::Written => state.completed("Bloomery lock publication"),
        PublicationOutcome::Unchanged => state.completed("Bloomery lock already current"),
    }

    let report = SyncReport {
        reconciled_locks: if plan.update_nix {
            vec!["Cargo.lock", "bloomery.lock", "flake.lock"]
        } else {
            vec!["Cargo.lock", "bloomery.lock"]
        }
        .into_iter()
        .map(str::to_owned)
        .collect(),
        updated_ecosystems: plan
            .selected_names()
            .into_iter()
            .map(str::to_owned)
            .collect(),
        skipped_updates: if plan.skip_nix {
            vec!["nix".to_owned()]
        } else {
            Vec::new()
        },
        completed_stages: state.completed.clone(),
        warnings: advisories.warnings,
        recommendations: advisories.recommendations,
        tool_stderr: state.tool_stderr.clone(),
    };
    write_completion(stdout, &plan, &state)?;
    Ok(report)
}

fn preflight(
    root: &Path,
    selection: &UpdateSelection,
    runner: &impl CommandRunner,
) -> Result<(UpdatePlan, LoadedConfig), SyncError> {
    let cargo_manifest = root.join("Cargo.toml");
    if !cargo_manifest.is_file() {
        return Err(SyncError::new(
            "preflight",
            format!(
                "workspace root must contain Cargo.toml (not found at '{}')",
                cargo_manifest.display()
            ),
        ));
    }

    let loaded_config = load_with_raw(root).map_err(|diagnostic| {
        SyncError::new(
            "configuration preflight",
            format_diagnostic(root, &diagnostic),
        )
    })?;
    let has_flake = root.join("flake.nix").is_file();
    let plan = UpdatePlan::new(selection, has_flake)
        .map_err(|error| SyncError::new("preflight", error.to_string()))?;

    if !runner.is_available("cargo") {
        return Err(SyncError::new(
            "tool preflight",
            "required executable 'cargo' was not found on PATH",
        ));
    }
    if plan.update_nix && !runner.is_available("nix") {
        return Err(SyncError::new(
            "tool preflight",
            "Nix updates were selected but required executable 'nix' was not found on PATH",
        ));
    }

    Ok((plan, loaded_config))
}

fn format_diagnostic(root: &Path, diagnostic: &bloomery_model::Diagnostic) -> String {
    let mut message = diagnostic.to_string();
    if let Some(location) = &diagnostic.location {
        let path = location.path.strip_prefix(root).unwrap_or(&location.path);
        message.push_str(&format!(" at {}", path.display()));
        if let Some(line) = location.line {
            message.push_str(&format!(":{line}"));
        }
    }
    for note in &diagnostic.notes {
        message.push_str(&format!("\n{note}"));
    }
    message
}

fn collect_advisories(loaded: &LoadedConfig) -> Advisories {
    match &loaded.raw {
        Some(raw) => Advisories {
            warnings: Vec::new(),
            recommendations: missing_recommendations(raw)
                .into_iter()
                .map(|recommendation| SyncRecommendation {
                    key: recommendation_key(recommendation),
                    benefit: recommendation.benefit.to_owned(),
                    guidance: recommendation.guidance.to_owned(),
                })
                .collect(),
        },
        None => Advisories {
            warnings: vec![".bloomery/config.toml is missing; create it to configure Bloomery's recommended features or explicitly disable them. Lock synchronization will continue.".to_owned()],
            recommendations: Vec::new(),
        },
    }
}

fn emit_advisories(advisories: &Advisories, stderr: &mut impl Write) -> Result<(), SyncError> {
    for warning in &advisories.warnings {
        writeln!(stderr, "warning: {warning}")
            .map_err(|error| SyncError::new("configuration warning", error.to_string()))?;
    }
    for recommendation in &advisories.recommendations {
        let message = format!(
            "recommended feature not yet configured: `{}` — {} {}",
            recommendation.key, recommendation.benefit, recommendation.guidance
        );
        writeln!(stderr, "recommendation: {message}")
            .map_err(|error| SyncError::new("recommendations", error.to_string()))?;
    }
    Ok(())
}

fn record_tool_stderr(state: &mut RunState, contents: &[u8]) {
    if !contents.is_empty() {
        state
            .tool_stderr
            .push(String::from_utf8_lossy(contents).trim_end().to_owned());
    }
}

fn cargo_metadata_arguments(manifest: &Path, locked: bool) -> Vec<OsString> {
    let mut arguments = vec![OsString::from("metadata")];
    if locked {
        arguments.push(OsString::from("--locked"));
    }
    arguments.extend([
        OsString::from("--manifest-path"),
        manifest.as_os_str().to_owned(),
        OsString::from("--format-version"),
        OsString::from("1"),
    ]);
    arguments
}

fn execute(
    runner: &mut impl CommandRunner,
    executable: &OsStr,
    arguments: &[OsString],
    root: &Path,
    stage: &str,
    state: &RunState,
) -> Result<CommandOutput, SyncError> {
    let output = runner
        .run(executable, arguments, root)
        .map_err(|error| state.failure(stage, format!("unable to invoke tool: {error}")))?;
    if !output.succeeded() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let details = if stderr.trim().is_empty() {
            output.status_description()
        } else {
            format!("{}:\n{}", output.status_description(), stderr.trim_end())
        };
        return Err(state.failure(stage, details));
    }
    Ok(output)
}

fn forward_stream(
    contents: &[u8],
    output: &mut impl Write,
    stage: &str,
    state: &RunState,
) -> Result<(), SyncError> {
    if contents.is_empty() {
        return Ok(());
    }
    output
        .write_all(contents)
        .map_err(|error| state.failure(stage, error.to_string()))?;
    if !contents.ends_with(b"\n") {
        writeln!(output).map_err(|error| state.failure(stage, error.to_string()))?;
    }
    Ok(())
}

fn write_progress(
    stdout: &mut impl Write,
    message: &str,
    stage: &str,
    state: &RunState,
) -> Result<(), SyncError> {
    writeln!(stdout, "{message}").map_err(|error| state.failure(stage, error.to_string()))
}

fn write_completion(
    stdout: &mut impl Write,
    plan: &UpdatePlan,
    state: &RunState,
) -> Result<(), SyncError> {
    let lock_summary = if plan.update_nix {
        "Cargo.lock, bloomery.lock, and flake.lock"
    } else {
        "Cargo.lock and bloomery.lock"
    };
    let selected = plan.selected_names();
    let updates = if selected.is_empty() {
        "none".to_owned()
    } else {
        selected.join(", ")
    };
    writeln!(
        stdout,
        "Successfully synchronized {lock_summary}; updated ecosystems: {updates}."
    )
    .map_err(|error| state.failure("completion output", error.to_string()))?;
    if plan.skip_nix {
        writeln!(
            stdout,
            "Skipped Nix input updates because flake.nix is absent."
        )
        .map_err(|error| state.failure("completion output", error.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CommandOutput, CommandRunner, SyncError, SyncReport, run};
    use crate::{UpdateSelection, parse_update_list};
    use bloomery_test_macros::bloomery;
    use std::collections::BTreeSet;
    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);
    const PACKAGE_A: &str = "fixture-a 1.0.0 (registry+https://example.invalid)";
    const PACKAGE_B: &str = "fixture-b 2.0.0 (registry+https://example.invalid)";

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let suffix = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "bloomery-sync-pipeline-{}-{suffix}",
                std::process::id()
            ));
            fs::create_dir_all(&root).expect("fixture directory");
            fs::write(
                root.join("Cargo.toml"),
                "[package]\nname = \"sync-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            )
            .expect("manifest");
            Self { root }
        }

        fn write_config(&self, config: &str) {
            let directory = self.root.join(".bloomery");
            fs::create_dir_all(&directory).expect("config directory");
            fs::write(directory.join("config.toml"), config).expect("config");
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[derive(Debug, Clone)]
    struct Invocation {
        executable: String,
        arguments: Vec<String>,
        working_directory: PathBuf,
    }

    struct FakeRunner {
        available: BTreeSet<String>,
        calls: Vec<Invocation>,
        fail_at: Option<usize>,
        failure_stderr: Vec<u8>,
        metadata: Vec<u8>,
    }

    impl Default for FakeRunner {
        fn default() -> Self {
            Self {
                available: BTreeSet::from(["cargo".to_owned(), "nix".to_owned()]),
                calls: Vec::new(),
                fail_at: None,
                failure_stderr: b"simulated tool failure".to_vec(),
                metadata: fixture_metadata(),
            }
        }
    }

    impl CommandRunner for FakeRunner {
        fn is_available(&self, executable: &str) -> bool {
            self.available.contains(executable)
        }

        fn run(
            &mut self,
            executable: &OsStr,
            arguments: &[OsString],
            working_directory: &Path,
        ) -> io::Result<CommandOutput> {
            let invocation = Invocation {
                executable: executable.to_string_lossy().into_owned(),
                arguments: arguments
                    .iter()
                    .map(|argument| argument.to_string_lossy().into_owned())
                    .collect(),
                working_directory: working_directory.to_path_buf(),
            };
            self.calls.push(invocation.clone());
            if self.fail_at == Some(self.calls.len()) {
                return Ok(CommandOutput {
                    code: Some(1),
                    stdout: Vec::new(),
                    stderr: self.failure_stderr.clone(),
                });
            }

            match invocation.executable.as_str() {
                "nix" => {
                    fs::write(working_directory.join("flake.lock"), "updated by nix\n")?;
                }
                "cargo"
                    if invocation
                        .arguments
                        .first()
                        .is_some_and(|arg| arg == "update") =>
                {
                    if !working_directory.join("Cargo.lock").exists() {
                        fs::write(working_directory.join("Cargo.lock"), "version = 4\n")?;
                    } else {
                        let mut lock = fs::read(working_directory.join("Cargo.lock"))?;
                        lock.extend_from_slice(b"# updated\n");
                        fs::write(working_directory.join("Cargo.lock"), lock)?;
                    }
                }
                "cargo"
                    if invocation
                        .arguments
                        .first()
                        .is_some_and(|arg| arg == "metadata")
                        && !working_directory.join("Cargo.lock").exists() =>
                {
                    fs::write(working_directory.join("Cargo.lock"), "version = 4\n")?;
                }
                "cargo"
                    if invocation
                        .arguments
                        .first()
                        .is_some_and(|arg| arg == "metadata") => {}
                _ => {}
            }

            let stdout = if invocation.executable == "cargo"
                && invocation
                    .arguments
                    .first()
                    .is_some_and(|arg| arg == "metadata")
            {
                self.metadata.clone()
            } else {
                Vec::new()
            };
            Ok(CommandOutput {
                code: Some(0),
                stdout,
                stderr: Vec::new(),
            })
        }
    }

    fn fixture_metadata() -> Vec<u8> {
        serde_json::json!({
            "packages": [
                {
                    "id": PACKAGE_A,
                    "name": "fixture-a",
                    "version": "1.0.0",
                    "edition": "2021",
                    "targets": [{"kind": ["lib"]}]
                },
                {
                    "id": PACKAGE_B,
                    "name": "fixture-b",
                    "version": "2.0.0",
                    "edition": "2021",
                    "targets": [{"kind": ["proc-macro"]}]
                }
            ],
            "resolve": {
                "nodes": [
                    {
                        "id": PACKAGE_A,
                        "features": ["default"],
                        "deps": [{"pkg": PACKAGE_B}]
                    },
                    {"id": PACKAGE_B, "features": [], "deps": []}
                ]
            }
        })
        .to_string()
        .into_bytes()
    }

    fn run_fixture(
        fixture: &Fixture,
        selection: &UpdateSelection,
        runner: &mut FakeRunner,
    ) -> (Result<SyncReport, SyncError>, String, String) {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let result = run(&fixture.root, selection, runner, &mut stdout, &mut stderr);
        (
            result,
            String::from_utf8(stdout).expect("UTF-8 stdout"),
            String::from_utf8(stderr).expect("UTF-8 stderr"),
        )
    }

    fn command_index(runner: &FakeRunner, executable: &str, first_arg: &str) -> usize {
        runner
            .calls
            .iter()
            .position(|call| {
                call.executable == executable
                    && call.arguments.first().is_some_and(|arg| arg == first_arg)
            })
            .expect("expected command")
    }

    #[test]
    #[bloomery("CLI-SYNC-LOCKS-001")]
    #[bloomery("CLI-SYNC-LOCKS-004")]
    #[bloomery("CLI-SYNC-LOCKS-007")]
    #[bloomery("CLI-SYNC-LOCKS-010")]
    #[bloomery("CLI-SYNC-LOCKS-027")]
    #[bloomery("CLI-SYNC-LOCKS-028")]
    #[bloomery("CLI-SYNC-LOCKS-029")]
    #[bloomery("CLI-SYNC-LOCKS-030")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-004")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-005")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-021")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-022")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-023")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-024")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-025")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-026")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-027")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-028")]
    fn normal_sync_creates_missing_locks_without_specs_or_config_directory() {
        let fixture = Fixture::new();
        let mut runner = FakeRunner::default();
        let (result, stdout, stderr) = run_fixture(&fixture, &UpdateSelection::None, &mut runner);

        result.expect("sync should succeed without pre-existing lock/spec/config files");
        assert!(fixture.root.join("Cargo.lock").is_file());
        assert!(fixture.root.join("bloomery.lock").is_file());
        assert!(!fixture.root.join(".bloomery").exists());
        assert_eq!(runner.calls.len(), 2);
        assert_eq!(runner.calls[0].arguments[0], "metadata");
        assert!(
            !runner.calls[0]
                .arguments
                .iter()
                .any(|arg| arg == "--locked")
        );
        assert!(
            runner.calls[1]
                .arguments
                .iter()
                .any(|arg| arg == "--locked")
        );
        assert!(runner.calls.iter().all(|call| call.executable == "cargo"));
        assert!(stdout.contains("Cargo.lock and bloomery.lock"));
        assert!(stderr.contains(".bloomery/config.toml is missing"));
        assert!(stderr.contains("Lock synchronization will continue"));
    }

    #[test]
    #[bloomery("CLI-SYNC-LOCKS-002")]
    #[bloomery("CLI-SYNC-LOCKS-011")]
    #[bloomery("CLI-SYNC-LOCKS-012")]
    #[bloomery("CLI-SYNC-LOCKS-013")]
    fn rust_update_runs_cargo_update_and_preserves_the_manifest() {
        let fixture = Fixture::new();
        fixture.write_config(
            "[scanners.rust]\nenabled = false\n\
             [scanners.playwright]\nenabled = false\n\
             [scanners.nix]\nenabled = false\n",
        );
        fs::write(fixture.root.join("Cargo.lock"), "version = 4\n").expect("initial lock");
        let manifest_before = fs::read(fixture.root.join("Cargo.toml")).expect("manifest bytes");
        let rust_only = parse_update_list("rust").expect("Rust update selection");
        let mut runner = FakeRunner::default();

        let (result, _stdout, _stderr) = run_fixture(&fixture, &rust_only, &mut runner);
        result.expect("Rust update should succeed");

        let update = command_index(&runner, "cargo", "update");
        let final_metadata = runner
            .calls
            .iter()
            .position(|call| {
                call.executable == "cargo"
                    && call.arguments.first().is_some_and(|arg| arg == "metadata")
                    && call.arguments.iter().any(|arg| arg == "--locked")
            })
            .expect("final locked metadata");
        assert!(update < final_metadata);
        assert!(
            runner.calls[update]
                .arguments
                .iter()
                .any(|arg| arg.ends_with("Cargo.toml"))
        );
        assert_eq!(
            fs::read(fixture.root.join("Cargo.toml")).expect("manifest"),
            manifest_before
        );
        assert!(runner.calls.iter().all(|call| call.executable != "nix"));
    }

    #[test]
    #[bloomery("CLI-SYNC-LOCKS-003")]
    #[bloomery("CLI-SYNC-LOCKS-014")]
    fn nix_updates_run_first_and_only_when_selected() {
        let fixture = Fixture::new();
        fixture.write_config(
            "[scanners.rust]\nenabled = false\n\
             [scanners.playwright]\nenabled = false\n\
             [scanners.nix]\nenabled = false\n",
        );
        fs::write(fixture.root.join("flake.nix"), "{ }").expect("flake");
        let nix_only = parse_update_list("nix").expect("Nix selection");
        let mut runner = FakeRunner::default();

        let (result, stdout, _stderr) = run_fixture(&fixture, &nix_only, &mut runner);
        result.expect("Nix-only update should succeed");

        assert_eq!(runner.calls[0].executable, "nix");
        assert_eq!(runner.calls[0].arguments, ["flake", "update"]);
        assert_eq!(runner.calls[0].working_directory, fixture.root);
        assert_eq!(runner.calls[1].arguments[0], "metadata");
        assert!(
            runner.calls[2]
                .arguments
                .iter()
                .any(|arg| arg == "--locked")
        );
        assert!(stdout.contains("updated ecosystems: nix"));
        assert!(fixture.root.join("flake.lock").exists());
    }

    #[test]
    #[bloomery("CLI-SYNC-LOCKS-011")]
    fn normal_reconciliation_preserves_an_existing_compatible_cargo_lock() {
        let fixture = Fixture::new();
        fixture.write_config(
            "[scanners.rust]\nenabled = false\n\
             [scanners.playwright]\nenabled = false\n\
             [scanners.nix]\nenabled = false\n",
        );
        let original_lock = b"version = 4\n# compatible pinned dependencies\n";
        fs::write(fixture.root.join("Cargo.lock"), original_lock).expect("Cargo lock");
        let mut runner = FakeRunner::default();

        let (result, _stdout, _stderr) = run_fixture(&fixture, &UpdateSelection::None, &mut runner);
        result.expect("normal reconciliation");
        assert_eq!(
            fs::read(fixture.root.join("Cargo.lock")).expect("Cargo lock"),
            original_lock
        );
        assert!(runner.calls.iter().all(|call| {
            !call
                .arguments
                .first()
                .is_some_and(|argument| argument == "update")
        }));
    }

    #[test]
    #[bloomery("CLI-SYNC-LOCKS-015")]
    #[bloomery("CLI-SYNC-LOCKS-016")]
    fn normal_sync_leaves_flake_lock_untouched_and_does_not_require_nix() {
        let fixture = Fixture::new();
        fixture.write_config(
            "[scanners.rust]\nenabled = false\n\
             [scanners.playwright]\nenabled = false\n\
             [scanners.nix]\nenabled = false\n",
        );
        fs::write(fixture.root.join("flake.nix"), "{ }").expect("flake");
        fs::write(fixture.root.join("flake.lock"), "pinned\n").expect("flake lock");
        let mut runner = FakeRunner::default();
        runner.available.remove("nix");

        let (result, _stdout, _stderr) = run_fixture(&fixture, &UpdateSelection::None, &mut runner);
        result.expect("Nix executable must be optional");
        assert!(runner.calls.iter().all(|call| call.executable != "nix"));
        assert_eq!(
            fs::read(fixture.root.join("flake.lock")).expect("flake lock"),
            b"pinned\n"
        );
    }

    #[test]
    #[bloomery("CLI-SYNC-INTERFACE-016")]
    fn explicit_nix_selection_without_a_flake_fails_before_any_command() {
        let fixture = Fixture::new();
        let selection = parse_update_list("nix").expect("Nix selection");
        let mut runner = FakeRunner::default();

        let error = run_fixture(&fixture, &selection, &mut runner)
            .0
            .expect_err("missing flake should be rejected");
        assert_eq!(error.stage(), "preflight");
        assert!(error.to_string().contains("flake.nix is absent"));
        assert!(runner.calls.is_empty());
        assert!(!fixture.root.join("Cargo.lock").exists());
    }

    #[test]
    #[bloomery("CLI-SYNC-LOCKS-008")]
    #[bloomery("CLI-SYNC-LOCKS-030")]
    fn absent_cargo_manifest_fails_preflight_without_invoking_tools() {
        let fixture = Fixture::new();
        fs::remove_file(fixture.root.join("Cargo.toml")).expect("remove manifest");
        let mut runner = FakeRunner::default();

        let (result, _stdout, _stderr) = run_fixture(&fixture, &UpdateSelection::None, &mut runner);
        let error = result.expect_err("missing manifest should fail");
        assert_eq!(error.stage(), "preflight");
        assert!(error.to_string().contains("Cargo.toml"));
        assert!(runner.calls.is_empty());
    }

    #[test]
    #[bloomery("CLI-SYNC-LOCKS-031")]
    fn unavailable_required_executables_fail_before_any_mutation() {
        let fixture = Fixture::new();
        let mut runner = FakeRunner::default();
        runner.available.remove("cargo");
        let result = run_fixture(&fixture, &UpdateSelection::None, &mut runner).0;
        assert!(
            result
                .expect_err("Cargo is required")
                .to_string()
                .contains("cargo")
        );
        assert!(runner.calls.is_empty());

        fs::write(fixture.root.join("flake.nix"), "{ }").expect("flake");
        runner.available.insert("cargo".to_owned());
        runner.available.remove("nix");
        let nix = parse_update_list("nix").expect("Nix selection");
        let result = run_fixture(&fixture, &nix, &mut runner).0;
        assert!(
            result
                .expect_err("Nix is required when selected")
                .to_string()
                .contains("nix")
        );
        assert!(runner.calls.is_empty());
    }

    #[test]
    #[bloomery("CLI-SYNC-LOCKS-032")]
    fn malformed_present_configuration_fails_preflight_before_tool_invocation() {
        let fixture = Fixture::new();
        fixture.write_config("[scanners\n");
        let mut runner = FakeRunner::default();

        let error = run_fixture(&fixture, &UpdateSelection::None, &mut runner)
            .0
            .expect_err("malformed config should fail");
        assert!(error.to_string().contains("configuration preflight"));
        assert!(error.to_string().contains("Unable to parse configuration"));
        assert!(runner.calls.is_empty());
    }

    #[cfg(unix)]
    #[test]
    #[bloomery("CLI-SYNC-LOCKS-032")]
    fn dangling_configuration_symlink_is_not_treated_as_missing() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.root.join(".bloomery")).expect("config parent");
        std::os::unix::fs::symlink("absent-target", fixture.root.join(".bloomery/config.toml"))
            .expect("dangling config link");
        let mut runner = FakeRunner::default();
        let error = run_fixture(&fixture, &UpdateSelection::None, &mut runner)
            .0
            .expect_err("dangling config should fail");
        assert!(error.to_string().contains("Unable to read configuration"));
        assert!(runner.calls.is_empty());
    }

    #[test]
    #[bloomery("CLI-SYNC-LOCKS-009")]
    #[bloomery("CLI-SYNC-LOCKS-033")]
    #[bloomery("CLI-SYNC-LOCKS-034")]
    fn tool_failures_identify_the_stage_keep_stderr_and_stop_the_pipeline() {
        let fixture = Fixture::new();
        fixture.write_config(
            "[scanners.rust]\nenabled = false\n\
             [scanners.playwright]\nenabled = false\n\
             [scanners.nix]\nenabled = false\n",
        );
        let mut runner = FakeRunner {
            fail_at: Some(1),
            ..FakeRunner::default()
        };
        let (result, _stdout, _stderr) = run_fixture(&fixture, &UpdateSelection::None, &mut runner);
        let error = result.expect_err("tool should fail");
        assert_eq!(error.exit_code(), 1);
        assert_eq!(error.stage(), "Cargo lock reconciliation");
        assert!(error.to_string().contains("simulated tool failure"));
        assert_eq!(runner.calls.len(), 1);
        assert!(!fixture.root.join("bloomery.lock").exists());
    }

    #[test]
    #[bloomery("CLI-SYNC-LOCKS-035")]
    #[bloomery("CLI-SYNC-LOCKS-036")]
    #[bloomery("CLI-SYNC-LOCKS-037")]
    #[bloomery("CLI-SYNC-LOCKS-042")]
    fn later_failures_report_completed_stages_keep_cargo_changes_and_give_recovery_guidance() {
        let fixture = Fixture::new();
        fixture.write_config(
            "[scanners.rust]\nenabled = false\n\
             [scanners.playwright]\nenabled = false\n\
             [scanners.nix]\nenabled = false\n",
        );
        let mut runner = FakeRunner {
            fail_at: Some(2),
            ..FakeRunner::default()
        };
        let error = run_fixture(&fixture, &UpdateSelection::None, &mut runner)
            .0
            .expect_err("final metadata failure");
        let message = error.to_string();
        assert_eq!(error.stage(), "final Cargo metadata");
        assert!(message.contains("Cargo lock reconciliation"));
        assert!(message.contains("partially synchronized"));
        assert!(message.contains("rerun `bloomery sync`"));
        assert!(fixture.root.join("Cargo.lock").exists());
        assert!(!fixture.root.join("bloomery.lock").exists());
        assert_eq!(runner.calls.len(), 2);
    }

    #[test]
    #[bloomery("CLI-SYNC-LOCKS-038")]
    #[bloomery("CLI-SYNC-LOCKS-039")]
    #[bloomery("CLI-SYNC-LOCKS-040")]
    fn successful_summary_reports_reconciled_locks_selected_updates_and_nix_skips() {
        let fixture = Fixture::new();
        fixture.write_config(
            "[scanners.rust]\nenabled = false\n\
             [scanners.playwright]\nenabled = false\n\
             [scanners.nix]\nenabled = false\n",
        );
        let mut runner = FakeRunner::default();
        let (result, stdout, _stderr) = run_fixture(&fixture, &UpdateSelection::All, &mut runner);
        result.expect("bare update should work without a flake");
        assert!(stdout.contains("Cargo.lock and bloomery.lock"));
        assert!(stdout.contains("updated ecosystems: rust"));
        assert!(stdout.contains("Skipped Nix input updates because flake.nix is absent"));
        assert_eq!(runner.calls[0].arguments[0], "update");
    }

    #[test]
    #[bloomery("CLI-SYNC-LOCKS-041")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-003")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-005")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-017")]
    fn recommendation_notices_are_advisory_sorted_and_written_to_stderr() {
        let fixture = Fixture::new();
        fixture.write_config("");
        let mut runner = FakeRunner::default();
        let (result, _stdout, stderr) = run_fixture(&fixture, &UpdateSelection::None, &mut runner);
        result.expect("recommendations are non-fatal");
        assert!(stderr.contains("recommendation: recommended feature not yet configured"));
        assert!(
            stderr.find("scanners.nix.enabled").unwrap()
                < stderr.find("scanners.playwright.enabled").unwrap()
        );
        assert!(
            stderr.find("scanners.playwright.enabled").unwrap()
                < stderr.find("scanners.rust.enabled").unwrap()
        );
        assert_eq!(
            stderr
                .matches("recommended feature not yet configured")
                .count(),
            3
        );
    }

    #[test]
    fn successful_sync_keeps_explicitly_disabled_recommendations_silent() {
        let fixture = Fixture::new();
        fixture.write_config(
            "[scanners.rust]\nenabled = false\n\
             [scanners.playwright]\nenabled = false\n\
             [scanners.nix]\nenabled = false\n",
        );
        let mut runner = FakeRunner::default();
        let config_before = fs::read(fixture.root.join(".bloomery/config.toml")).expect("config");
        let (result, _stdout, stderr) = run_fixture(&fixture, &UpdateSelection::None, &mut runner);
        result.expect("sync should succeed");
        assert_eq!(
            fs::read(fixture.root.join(".bloomery/config.toml")).expect("config"),
            config_before
        );
        assert!(!stderr.contains("recommended feature"));
        assert!(!stderr.contains("missing"));
    }

    #[test]
    fn colliding_metadata_fails_before_bloomery_lock_publication() {
        let fixture = Fixture::new();
        fixture.write_config(
            "[scanners.rust]\nenabled = false\n\
             [scanners.playwright]\nenabled = false\n\
             [scanners.nix]\nenabled = false\n",
        );
        fs::write(fixture.root.join("bloomery.lock"), "previous lock\n")
            .expect("old Bloomery lock");
        let mut runner = FakeRunner::default();
        let mut metadata: serde_json::Value =
            serde_json::from_slice(&runner.metadata).expect("fixture metadata");
        let mut colliding = metadata["packages"][0].clone();
        colliding["id"] = serde_json::Value::String(
            "fixture-a 1.0.0 (git+https://example.invalid#revision)".to_owned(),
        );
        metadata["packages"]
            .as_array_mut()
            .expect("packages")
            .push(colliding);
        metadata["resolve"]["nodes"]
            .as_array_mut()
            .expect("nodes")
            .push(serde_json::json!({
                "id": "fixture-a 1.0.0 (git+https://example.invalid#revision)",
                "features": [],
                "deps": []
            }));
        runner.metadata = serde_json::to_vec(&metadata).expect("serialize metadata");

        let error = run_fixture(&fixture, &UpdateSelection::None, &mut runner)
            .0
            .expect_err("collision should be rejected");
        assert_eq!(error.stage(), "Bloomery lock generation");
        assert!(error.to_string().contains("unsupported Cargo resolution"));
        assert_eq!(
            fs::read(fixture.root.join("bloomery.lock")).expect("old lock preserved"),
            b"previous lock\n"
        );
    }

    #[test]
    fn preflight_and_reconciliation_do_not_modify_the_manifest_or_spec_tree() {
        let fixture = Fixture::new();
        let manifest = fs::read(fixture.root.join("Cargo.toml")).expect("manifest");
        let mut runner = FakeRunner::default();
        let (result, _stdout, _stderr) = run_fixture(&fixture, &UpdateSelection::None, &mut runner);
        result.expect("sync");
        assert_eq!(
            fs::read(fixture.root.join("Cargo.toml")).expect("manifest"),
            manifest
        );
        assert!(!fixture.root.join(".bloomery/specs").exists());
    }
}
