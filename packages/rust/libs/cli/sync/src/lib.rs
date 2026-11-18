mod lockfile;
mod metadata;
#[cfg(test)]
mod naming_tests;
mod nix_migration;
mod recommendations;
mod runner;
mod selection;
mod traversal;

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
use std::path::{Path, PathBuf};

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
    flake: PathBuf,
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

impl RunState {
    fn completed(&mut self, stage: &str) {
        self.completed.push(self.stage_name(stage));
    }

    fn failure(&self, stage: &str, message: impl Into<String>) -> SyncError {
        SyncError {
            stage: self.stage_name(stage),
            message: message.into(),
            completed_stages: self.completed.clone(),
            may_be_partially_synchronized: self.may_be_partially_synchronized,
        }
    }

    fn stage_name(&self, stage: &str) -> String {
        if self.flake.as_os_str().is_empty() {
            stage.to_owned()
        } else {
            format!("{}: {stage}", self.flake.display())
        }
    }
}

/// Reconcile locks across `root` and its configured sub-flakes, optionally
/// upgrading dependencies in the selected ecosystems.
pub fn run(
    root: &Path,
    selection: &UpdateSelection,
    runner: &mut impl CommandRunner,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<SyncReport, SyncError> {
    let (plan, loaded_config) = preflight(root, selection, runner)?;
    let flakes = traversal::discover(root, &loaded_config)?;
    let recommendations = collect_recommendations(&loaded_config);
    emit_recommendations(&recommendations, stderr)?;

    let mut state = RunState::default();
    let mut reconciled_locks = Vec::new();
    let mut reconciled_nix_lock = false;
    for flake in &flakes {
        if !flake.has_cargo && !plan.update_nix {
            continue;
        }
        state.flake.clone_from(&flake.relative);
        if !state.flake.as_os_str().is_empty() {
            let path = state.flake.display();
            write_progress(stdout, format_args!("Synchronizing flake {path}"), &state)?;
        }
        sync_flake(flake, &plan, runner, stdout, stderr, &mut state)?;
        if flake.has_cargo {
            reconciled_locks
                .extend(["Cargo.lock", "bloomery.lock"].map(|lock| flake.lock_path(lock)));
        }
        if plan.update_nix && flake.root.join("flake.lock").is_file() {
            reconciled_locks.push(flake.lock_path("flake.lock"));
            reconciled_nix_lock = true;
        }
    }

    state.flake.clear();
    write_completion(stdout, &plan, reconciled_nix_lock, &state)?;
    Ok(SyncReport {
        reconciled_locks,
        updated_ecosystems: plan
            .selected_names()
            .into_iter()
            .map(str::to_owned)
            .collect(),
        skipped_updates: Vec::new(),
        completed_stages: state.completed,
        warnings: Vec::new(),
        recommendations,
        tool_stderr: state.tool_stderr,
    })
}

fn sync_flake(
    flake: &traversal::Flake,
    plan: &UpdatePlan,
    runner: &mut impl CommandRunner,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    state: &mut RunState,
) -> Result<(), SyncError> {
    let root = &flake.root;
    if plan.update_nix {
        write_progress(stdout, "Updating Nix flake inputs", state)?;
        state.may_be_partially_synchronized = true;
        let output = execute(
            runner,
            OsStr::new("nix"),
            &[OsString::from("flake"), OsString::from("update")],
            root,
            "Nix input update",
            stderr,
            state,
        )?;
        forward_stream(&output.stdout, stdout, "Nix input update output", state)?;
    }

    if !flake.has_cargo {
        return Ok(());
    }

    let cargo_manifest = root.join("Cargo.toml");
    let (progress, stage, arguments) = if plan.update_rust {
        (
            "Updating Rust dependencies",
            "Rust dependency update",
            vec![
                "update".into(),
                "--manifest-path".into(),
                cargo_manifest.as_os_str().into(),
            ],
        )
    } else {
        (
            "Reconciling Cargo.lock with workspace manifests",
            "Cargo lock reconciliation",
            cargo_metadata_arguments(&cargo_manifest, false),
        )
    };
    write_progress(stdout, progress, state)?;
    state.may_be_partially_synchronized = true;
    let output = execute(
        runner,
        OsStr::new("cargo"),
        &arguments,
        root,
        stage,
        stderr,
        state,
    )?;
    if plan.update_rust {
        forward_stream(&output.stdout, stdout, "Cargo update output", state)?;
    }

    write_progress(stdout, "Reading final locked Cargo metadata", state)?;
    let final_metadata_arguments = cargo_metadata_arguments(&cargo_manifest, true);
    let final_metadata = execute(
        runner,
        OsStr::new("cargo"),
        &final_metadata_arguments,
        root,
        "final Cargo metadata",
        stderr,
        state,
    )?;

    write_progress(stdout, "Generating and validating bloomery.lock", state)?;
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

    Ok(())
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
    if !bloomery_cli_types::flake_present(root) {
        return Err(SyncError::new(
            "preflight",
            bloomery_cli_types::MISSING_FLAKE_MESSAGE,
        ));
    }

    let loaded_config = load_with_raw(root).map_err(|diagnostic| {
        SyncError::new(
            "configuration preflight",
            format_diagnostic(root, &diagnostic),
        )
    })?;
    let plan = UpdatePlan::new(selection);

    let tools: &[_] = if plan.update_nix {
        &["cargo", "nix"]
    } else {
        &["cargo"]
    };
    for tool in tools {
        if !runner.is_available(tool) {
            return Err(SyncError::new(
                "tool preflight",
                format!("required executable '{tool}' was not found on PATH"),
            ));
        }
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

fn collect_recommendations(loaded: &LoadedConfig) -> Vec<SyncRecommendation> {
    loaded
        .raw
        .iter()
        .flat_map(missing_recommendations)
        .map(|recommendation| SyncRecommendation {
            key: recommendation_key(recommendation),
            benefit: recommendation.benefit.to_owned(),
            guidance: recommendation.guidance.to_owned(),
        })
        .collect()
}

fn emit_recommendations(
    recommendations: &[SyncRecommendation],
    stderr: &mut impl Write,
) -> Result<(), SyncError> {
    for recommendation in recommendations {
        let message = format!(
            "recommended feature not yet configured: `{}` — {} {}",
            recommendation.key, recommendation.benefit, recommendation.guidance
        );
        writeln!(stderr, "recommendation: {message}")
            .map_err(|error| SyncError::new("recommendations", error.to_string()))?;
    }
    Ok(())
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
    stderr: &mut impl Write,
    state: &mut RunState,
) -> Result<CommandOutput, SyncError> {
    let output = runner
        .run(executable, arguments, root)
        .map_err(|error| state.failure(stage, format!("unable to invoke tool: {error}")))?;
    let tool_stderr = String::from_utf8_lossy(&output.stderr);
    if !output.succeeded() {
        let details = if tool_stderr.trim().is_empty() {
            output.status_description()
        } else {
            format!(
                "{}:\n{}",
                output.status_description(),
                tool_stderr.trim_end()
            )
        };
        return Err(state.failure(stage, details));
    }
    state.completed(stage);
    if !output.stderr.is_empty() {
        state.tool_stderr.push(tool_stderr.trim_end().to_owned());
        forward_stream(&output.stderr, stderr, stage, state)?;
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
    message: impl fmt::Display,
    state: &RunState,
) -> Result<(), SyncError> {
    writeln!(stdout, "==> {message}...")
        .map_err(|error| state.failure("progress output", error.to_string()))
}

fn write_completion(
    stdout: &mut impl Write,
    plan: &UpdatePlan,
    reconciled_nix_lock: bool,
    state: &RunState,
) -> Result<(), SyncError> {
    let lock_summary = if reconciled_nix_lock {
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
    .map_err(|error| state.failure("completion output", error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{CommandOutput, CommandRunner, SyncError, SyncReport, run};
    use crate::{UpdateSelection, parse_update_list};
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
            let temp_dir = std::env::temp_dir().canonicalize().expect("temp directory");
            let root = temp_dir.join(format!(
                "bloomery-sync-pipeline-{}-{suffix}",
                std::process::id()
            ));
            Self::create(root, true)
        }

        fn child(&self, relative: &str, has_cargo: bool) -> Self {
            Self::create(self.root.join(relative), has_cargo)
        }

        fn create(root: PathBuf, has_cargo: bool) -> Self {
            fs::create_dir_all(&root).expect("fixture directory");
            if has_cargo {
                fs::write(
                    root.join("Cargo.toml"),
                    "[package]\nname = \"sync-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
                )
                .expect("manifest");
            }
            fs::write(root.join("flake.nix"), "{ }\n").expect("flake");
            Self { root }
        }

        fn write_config(&self, config: &str) {
            let directory = self.root.join(".bloomery");
            fs::create_dir_all(&directory).expect("config directory");
            fs::write(
                directory.join("config.toml"),
                format!("version = 2\n{config}"),
            )
            .expect("config");
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
                    "targets": [{"kind": ["lib"], "name": "fixture_a"}]
                },
                {
                    "id": PACKAGE_B,
                    "name": "fixture-b",
                    "version": "2.0.0",
                    "edition": "2021",
                    "targets": [{"kind": ["proc-macro"], "name": "fixture_b"}]
                }
            ],
            "resolve": {
                "nodes": [
                    {
                        "id": PACKAGE_A,
                        "features": ["default"],
                        "deps": [{"pkg": PACKAGE_B, "name": "fixture_b", "dep_kinds": [{"kind": null, "target": null}]}]
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
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECURSION-001"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECURSION-002"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECURSION-006"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECURSION-010"))]
    fn normal_sync_reconciles_nested_workspaces_and_reports_their_locks() {
        let fixture = Fixture::new();
        let child = fixture.child("children/first", true);
        let nested = child.child("more/second", true);
        let unrelated = fixture.child("unconfigured", true);
        fixture.write_config("[flakes.first]\npath = 'children/first'\n");
        child.write_config("[flakes.second]\npath = 'more/second'\n");
        let mut runner = FakeRunner::default();
        runner.available.remove("nix");

        let (report, stdout, _) = run_fixture(&fixture, &UpdateSelection::None, &mut runner);
        let report = report.expect("recursive sync succeeds");
        for workspace in [&fixture, &child, &nested] {
            assert!(workspace.root.join("Cargo.lock").is_file());
            assert!(workspace.root.join("bloomery.lock").is_file());
        }
        assert!(!nested.root.join(".bloomery/config.toml").exists());
        assert!(!unrelated.root.join("Cargo.lock").exists());
        assert_eq!(
            report.reconciled_locks,
            [
                "Cargo.lock",
                "bloomery.lock",
                "children/first/Cargo.lock",
                "children/first/bloomery.lock",
                "children/first/more/second/Cargo.lock",
                "children/first/more/second/bloomery.lock",
            ]
        );
        assert!(stdout.contains("Synchronizing flake children/first/more/second"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECURSION-003"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECURSION-004"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECURSION-005"))]
    fn updates_reach_nested_flakes_and_workspaces_once() {
        let fixture = Fixture::new();
        let child = fixture.child("first", true);
        let nested = child.child("second", true);
        let nix_only = child.child("nix-only", false);
        fixture.write_config("[flakes.a]\npath = 'first'\n[flakes.b]\npath = './first'\n");
        child.write_config("[flakes.a]\npath = 'second'\n[flakes.b]\npath = 'nix-only'\n");

        for (selection, nix, rust) in [
            ("nix,rust", true, true),
            ("nix", true, false),
            ("rust", false, true),
        ] {
            let mut runner = FakeRunner::default();
            let selection = parse_update_list(selection).expect("update selection");
            run_fixture(&fixture, &selection, &mut runner)
                .0
                .expect("recursive updates succeed");
            let calls = |executable: &str, first_arg: &str| {
                runner
                    .calls
                    .iter()
                    .filter(|call| call.executable == executable && call.arguments[0] == first_arg)
                    .map(|call| call.working_directory.as_path())
                    .collect::<Vec<_>>()
            };
            let nix_roots =
                [&fixture, &child, &nested, &nix_only].map(|flake| flake.root.as_path());
            let rust_roots = [&fixture, &child, &nested].map(|flake| flake.root.as_path());
            assert_eq!(
                calls("nix", "flake"),
                if nix { &nix_roots[..] } else { &[] }
            );
            assert_eq!(
                calls("cargo", "update"),
                if rust { &rust_roots[..] } else { &[] }
            );
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECURSION-007"))]
    fn invalid_nested_flake_config_fails_before_running_tools() {
        let fixture = Fixture::new();
        let child = fixture.child("first", true);
        let nested = child.child("second", true);
        fixture.write_config("[flakes.a]\npath = 'first'\n");
        child.write_config("[flakes.b]\npath = 'second'\n");
        nested.write_config("invalid=\n");
        let mut runner = FakeRunner::default();

        let error = run_fixture(&fixture, &UpdateSelection::All, &mut runner)
            .0
            .expect_err("invalid config");
        assert!(error.message().contains("first/second"));
        assert!(error.message().contains("config.toml"));
        assert!(runner.calls.is_empty());
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECURSION-008"))]
    fn invalid_sub_flake_paths_fail_before_running_tools() {
        let fixture = Fixture::new();
        let not_a_flake = fixture.child("not-a-flake", true);
        fs::remove_file(not_a_flake.root.join("flake.nix")).expect("remove flake file");
        let invalid_paths = ["", "missing", "not-a-flake", "../outside", "."];
        #[cfg(unix)]
        let outside = Fixture::new();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside.root, fixture.root.join("escaped"))
            .expect("symlink to another flake");
        #[cfg(unix)]
        let invalid_paths = invalid_paths.into_iter().chain(["escaped"]);
        for path in invalid_paths {
            fixture.write_config(&format!("[flakes.invalid]\npath = '{path}'\n"));
            let mut runner = FakeRunner::default();
            let error = run_fixture(&fixture, &UpdateSelection::All, &mut runner)
                .0
                .expect_err("invalid path");
            assert!(error.message().contains("[flakes.invalid].path"));
            assert!(runner.calls.is_empty());
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECURSION-009"))]
    fn descendant_tool_failure_labels_the_stage_and_stops_traversal() {
        let fixture = Fixture::new();
        let child = fixture.child("first", true);
        let nested = child.child("second", true);
        let sibling = fixture.child("third", true);
        fixture.write_config("[flakes.a]\npath = 'first'\n[flakes.b]\npath = 'third'\n");
        child.write_config("[flakes.a]\npath = 'second'\n");
        let mut runner = FakeRunner {
            fail_at: Some(5),
            ..FakeRunner::default()
        };

        let (result, stdout, _) = run_fixture(&fixture, &UpdateSelection::None, &mut runner);
        let error = result.expect_err("nested Cargo reconciliation fails");
        assert_eq!(error.stage(), "first/second: Cargo lock reconciliation");
        assert!(
            error
                .completed_stages()
                .iter()
                .any(|stage| stage == "first: Bloomery lock publication")
        );
        assert!(error.may_be_partially_synchronized());
        assert!(!nested.root.join("bloomery.lock").exists());
        assert!(!sibling.root.join("Cargo.lock").exists());
        assert!(!stdout.contains("Successfully synchronized"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-001"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-004"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-007"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-010"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-027"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-028"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-029"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-030"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECOMMENDATIONS-005"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECOMMENDATIONS-025"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECOMMENDATIONS-026"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECOMMENDATIONS-027"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECOMMENDATIONS-028"))]
    fn normal_sync_creates_missing_locks_without_specs_directory() {
        let fixture = Fixture::new();
        fixture.write_config("");
        let config_before = fs::read(fixture.root.join(".bloomery/config.toml")).expect("config");
        let mut runner = FakeRunner::default();
        let (result, stdout, stderr) = run_fixture(&fixture, &UpdateSelection::None, &mut runner);

        result.expect("sync should succeed with a config file present");
        assert!(fixture.root.join("Cargo.lock").is_file());
        assert!(fixture.root.join("bloomery.lock").is_file());
        assert!(!fixture.root.join(".bloomery/specs").exists());
        assert_eq!(
            fs::read(fixture.root.join(".bloomery/config.toml")).expect("config"),
            config_before
        );
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
        assert!(!stderr.contains("missing"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECOMMENDATIONS-004"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECOMMENDATIONS-021"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECOMMENDATIONS-022"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECOMMENDATIONS-023"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECOMMENDATIONS-024"))]
    fn absent_configuration_fails_sync_before_lock_operations() {
        let fixture = Fixture::new();
        let mut runner = FakeRunner::default();
        let (result, _stdout, _stderr) = run_fixture(&fixture, &UpdateSelection::None, &mut runner);

        let error = result.expect_err("missing config should fail");
        assert_eq!(error.stage(), "configuration preflight");
        assert!(error.to_string().contains("config.toml"));
        assert!(runner.calls.is_empty());
        assert!(!fixture.root.join("Cargo.lock").exists());
        assert!(!fixture.root.join("bloomery.lock").exists());
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-002"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-011"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-012"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-013"))]
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
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-003"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-014"))]
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
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-011"))]
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
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-015"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-016"))]
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
    #[cfg_attr(any(), bloomery("CLI-SYNC-INTERFACE-016"))]
    fn explicit_nix_selection_without_a_flake_fails_before_any_command() {
        let fixture = Fixture::new();
        fs::remove_file(fixture.root.join("flake.nix")).expect("remove flake");
        let selection = parse_update_list("nix").expect("Nix selection");
        let mut runner = FakeRunner::default();

        let error = run_fixture(&fixture, &selection, &mut runner)
            .0
            .expect_err("missing flake should be rejected");
        assert_eq!(error.stage(), "preflight");
        assert!(error.to_string().contains("flake.nix"));
        assert!(runner.calls.is_empty());
        assert!(!fixture.root.join("Cargo.lock").exists());
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-040"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-INTERFACE-010"))]
    fn missing_root_flake_blocks_sync_before_lock_operations() {
        let fixture = Fixture::new();
        fs::remove_file(fixture.root.join("flake.nix")).expect("remove flake");
        let mut runner = FakeRunner::default();

        let (result, stdout, _stderr) = run_fixture(&fixture, &UpdateSelection::All, &mut runner);
        let error = result.expect_err("missing flake should fail");
        assert_eq!(error.stage(), "preflight");
        assert!(error.to_string().contains("flake.nix"));
        assert!(runner.calls.is_empty());
        assert!(!fixture.root.join("Cargo.lock").exists());
        assert!(!fixture.root.join("bloomery.lock").exists());
        assert!(!stdout.contains("Successfully synchronized"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-008"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-030"))]
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
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-031"))]
    fn unavailable_required_executables_fail_before_any_mutation() {
        let fixture = Fixture::new();
        fixture.write_config("");
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
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-032"))]
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
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-032"))]
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
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-009"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-033"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-034"))]
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
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-035"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-036"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-037"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-042"))]
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
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-038"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-039"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-INTERFACE-009"))]
    fn successful_summary_reports_reconciled_locks_and_selected_updates() {
        let fixture = Fixture::new();
        fixture.write_config(
            "[scanners.rust]\nenabled = false\n\
             [scanners.playwright]\nenabled = false\n\
             [scanners.nix]\nenabled = false\n",
        );
        let mut runner = FakeRunner::default();
        let (result, stdout, _stderr) = run_fixture(&fixture, &UpdateSelection::All, &mut runner);
        result.expect("bare update should select both ecosystems");
        assert!(stdout.contains("Cargo.lock, bloomery.lock, and flake.lock"));
        assert!(stdout.contains("updated ecosystems: nix, rust"));
        assert!(!stdout.contains("Skipped"));
        assert_eq!(runner.calls[0].executable, "nix");
        assert_eq!(runner.calls[0].arguments, ["flake", "update"]);
        assert!(fixture.root.join("flake.lock").exists());
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-LOCKS-041"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECOMMENDATIONS-003"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECOMMENDATIONS-005"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-RECOMMENDATIONS-017"))]
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
        fixture.write_config("");
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
