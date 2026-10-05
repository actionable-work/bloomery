//! `bloomery init`: scaffold a new Bloomery flake and Rust workspace.

mod template;

use bloomery_sync::{CommandRunner, SystemCommandRunner, UpdateSelection};
use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub use template::TEMPLATE_NAMES;

/// A resolved init request.
#[derive(Debug, Clone)]
pub struct InitRequest {
    pub directory: PathBuf,
    pub template: Option<String>,
    pub force: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitErrorKind {
    Usage,
    Operational,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitError {
    kind: InitErrorKind,
    message: String,
}

impl InitError {
    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            kind: InitErrorKind::Usage,
            message: message.into(),
        }
    }

    pub fn operational(message: impl Into<String>) -> Self {
        Self {
            kind: InitErrorKind::Operational,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> InitErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn exit_code(&self) -> u8 {
        match self.kind {
            InitErrorKind::Usage => 2,
            InitErrorKind::Operational => 1,
        }
    }
}

impl fmt::Display for InitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for InitError {}

/// The outcome of a successful scaffold and lock pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitReport {
    pub directory: PathBuf,
    pub template: String,
    pub created: Vec<PathBuf>,
    pub lockfiles: Vec<String>,
}

/// The tool preflight and lock stage, injectable for tests.
pub trait LockStage {
    fn missing_tools(&self) -> Vec<&'static str>;

    fn lock<O: Write, E: Write>(
        &mut self,
        root: &Path,
        stdout: &mut O,
        stderr: &mut E,
    ) -> Result<Vec<String>, InitError>;
}

/// Production lock stage that reuses the `bloomery sync` reconciliation.
pub struct SyncLockStage<R> {
    pub runner: R,
}

impl SyncLockStage<SystemCommandRunner> {
    pub fn system() -> Self {
        Self {
            runner: SystemCommandRunner,
        }
    }
}

impl<R: CommandRunner> LockStage for SyncLockStage<R> {
    fn missing_tools(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if !self.runner.is_available("cargo") {
            missing.push("cargo");
        }
        if !self.runner.is_available("nix") {
            missing.push("nix");
        }
        missing
    }

    fn lock<O: Write, E: Write>(
        &mut self,
        root: &Path,
        stdout: &mut O,
        stderr: &mut E,
    ) -> Result<Vec<String>, InitError> {
        bloomery_sync::run(
            root,
            &UpdateSelection::All,
            &mut self.runner,
            stdout,
            stderr,
        )
        .map_err(|error| InitError::operational(format!("lock stage failed: {error}")))?;
        Ok(vec![
            "Cargo.lock".to_owned(),
            "bloomery.lock".to_owned(),
            "flake.lock".to_owned(),
        ])
    }
}

/// Scaffolds the selected template into the target and locks the workspace.
pub fn run<L: LockStage, O: Write, E: Write>(
    request: &InitRequest,
    lock: &mut L,
    stdout: &mut O,
    stderr: &mut E,
) -> Result<InitReport, InitError> {
    let template = template::resolve(request.template.as_deref())?;
    let project = project_name(&request.directory)?;
    guard_target(&request.directory, request.force, stderr)?;
    let missing = lock.missing_tools();
    if !missing.is_empty() {
        return Err(InitError::operational(format!(
            "required executable(s) not found on PATH: {}",
            missing.join(", ")
        )));
    }
    let created = scaffold(&request.directory, template, &project)?;
    let lockfiles = lock.lock(&request.directory, stdout, stderr)?;
    Ok(InitReport {
        directory: request.directory.clone(),
        template: template.to_owned(),
        created,
        lockfiles,
    })
}

fn project_name(target: &Path) -> Result<String, InitError> {
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            InitError::usage("target directory has no usable name to derive a project name")
        })?;
    let normalized: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let valid = normalized
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric())
        && normalized
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_');
    if !valid {
        return Err(InitError::usage(format!(
            "cannot derive a valid Cargo package name from '{name}'"
        )));
    }
    Ok(normalized)
}

fn guard_target<W: Write>(target: &Path, force: bool, stderr: &mut W) -> Result<(), InitError> {
    if force {
        return Ok(());
    }
    match std::fs::read_dir(target) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                writeln!(
                    stderr,
                    "warning: target directory '{}' is not empty; use --force to overwrite",
                    target.display()
                )
                .map_err(|error| InitError::operational(error.to_string()))?;
                return Err(InitError::operational(format!(
                    "refusing to scaffold into non-empty directory '{}'",
                    target.display()
                )));
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(InitError::operational(format!(
            "unable to read target directory '{}': {error}",
            target.display()
        ))),
    }
}

fn scaffold(root: &Path, template: &'static str, project: &str) -> Result<Vec<PathBuf>, InitError> {
    std::fs::create_dir_all(root).map_err(|error| {
        InitError::operational(format!("unable to create '{}': {error}", root.display()))
    })?;
    let mut created = Vec::new();
    for (relative, content) in template::files(template) {
        let relative = relative.replace("{{project}}", project);
        let rendered = content.replace("{{project}}", project);
        let path = root.join(&relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                InitError::operational(format!("unable to create '{}': {error}", parent.display()))
            })?;
        }
        std::fs::write(&path, rendered).map_err(|error| {
            InitError::operational(format!("unable to write '{}': {error}", path.display()))
        })?;
        created.push(PathBuf::from(relative));
    }
    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::{InitError, InitRequest, LockStage, SyncLockStage, TEMPLATE_NAMES, run};
    use bloomery_sync::{CommandOutput, CommandRunner};
    use std::collections::BTreeMap;
    use std::ffi::{OsStr, OsString};
    use std::io::{self, Write};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_root(tag: &str) -> PathBuf {
        let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "bloomery-init-{tag}-{}-{suffix}",
            std::process::id()
        ))
    }

    fn request(root: &Path, template: Option<&str>, force: bool) -> InitRequest {
        InitRequest {
            directory: root.to_path_buf(),
            template: template.map(str::to_owned),
            force,
        }
    }

    #[derive(Default)]
    struct FakeLock {
        missing: Vec<&'static str>,
        fail: bool,
        locked: bool,
        files_present_at_lock: bool,
    }

    impl LockStage for FakeLock {
        fn missing_tools(&self) -> Vec<&'static str> {
            self.missing.clone()
        }

        fn lock<O: Write, E: Write>(
            &mut self,
            root: &Path,
            _stdout: &mut O,
            _stderr: &mut E,
        ) -> Result<Vec<String>, InitError> {
            self.locked = true;
            self.files_present_at_lock =
                root.join("Cargo.toml").is_file() && root.join("flake.nix").is_file();
            if self.fail {
                return Err(InitError::operational("lock stage failed: simulated"));
            }
            Ok(vec![
                "Cargo.lock".to_owned(),
                "bloomery.lock".to_owned(),
                "flake.lock".to_owned(),
            ])
        }
    }

    fn template_files(name: &'static str) -> BTreeMap<&'static str, &'static str> {
        super::template::files(name).collect()
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INIT-COMMAND-004"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-COMMAND-009"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-COMMAND-010"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-TEMPLATES-001"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-TEMPLATES-009"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-TEMPLATES-010"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-014"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-015"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-023"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-LOCK-004"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-LOCK-008"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-OUTPUT-003"))]
    fn scaffolds_the_basic_template_into_a_missing_target() {
        let root = temp_root("basic");
        let target = root.join("app");
        let mut lock = FakeLock::default();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let report = run(
            &request(&target, None, false),
            &mut lock,
            &mut stdout,
            &mut stderr,
        )
        .expect("init succeeds");
        assert_eq!(report.template, "basic");
        assert!(report.lockfiles.contains(&"Cargo.lock".to_owned()));
        assert!(!report.created.is_empty());
        assert!(lock.locked);
        assert!(lock.files_present_at_lock, "lock runs after scaffolding");
        let manifest = std::fs::read_to_string(target.join("Cargo.toml")).expect("manifest");
        assert!(!manifest.contains("{{project}}"));
        assert!(manifest.contains("crates/app"));
        assert!(target.join("crates/app_core/src/lib.rs").is_file());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INIT-TEMPLATES-005"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-TEMPLATES-002"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-016"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-017"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-024"))]
    fn scaffolds_the_axum_template() {
        let root = temp_root("axum");
        let target = root.join("app");
        let mut lock = FakeLock::default();
        let report = run(
            &request(&target, Some("axum"), false),
            &mut lock,
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .expect("init succeeds");
        assert_eq!(report.template, "axum");
        assert!(target.join("crates/app_server/src/lib.rs").is_file());
        assert!(target.join("crates/app/src/main.rs").is_file());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INIT-TEMPLATES-006"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-018"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-019"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-020"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-025"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-026"))]
    fn scaffolds_the_topcoat_template() {
        let root = temp_root("topcoat");
        let target = root.join("app");
        let mut lock = FakeLock::default();
        let report = run(
            &request(&target, Some("topcoat"), false),
            &mut lock,
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .expect("init succeeds");
        assert_eq!(report.template, "topcoat");
        assert!(target.join("crates/app_ui/src/lib.rs").is_file());
        assert!(target.join("crates/app_server/src/lib.rs").is_file());
        assert!(target.join("crates/app/src/main.rs").is_file());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INIT-TEMPLATES-003"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-TEMPLATES-007"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-COMMAND-007"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-OUTPUT-005"))]
    fn unknown_template_is_a_usage_error_without_writing() {
        let root = temp_root("unknown-template");
        let mut lock = FakeLock::default();
        let error = run(
            &request(&root, Some("nope"), false),
            &mut lock,
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .expect_err("unknown template");
        assert_eq!(error.kind(), super::InitErrorKind::Usage);
        assert_eq!(error.exit_code(), 2);
        assert!(!lock.locked);
        assert!(!root.exists());
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INIT-TEMPLATES-011"))]
    fn invalid_project_name_fails_before_writing() {
        let root = temp_root("invalid-name");
        let target = root.join("!!!");
        let mut lock = FakeLock::default();
        let error = run(
            &request(&target, None, false),
            &mut lock,
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .expect_err("invalid name");
        assert_eq!(error.exit_code(), 2);
        assert!(!root.exists());
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INIT-COMMAND-005"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-OUTPUT-006"))]
    fn non_empty_target_warns_and_refuses_without_force() {
        let root = temp_root("non-empty");
        std::fs::create_dir_all(&root).expect("target");
        std::fs::write(root.join("existing.txt"), "keep").expect("existing file");
        let mut lock = FakeLock::default();
        let mut stderr = Vec::new();
        let error = run(
            &request(&root, None, false),
            &mut lock,
            &mut Vec::new(),
            &mut stderr,
        )
        .expect_err("refuses");
        assert_eq!(error.exit_code(), 1);
        assert!(!lock.locked);
        let warning = String::from_utf8(stderr).expect("stderr");
        assert!(warning.contains("warning"));
        assert!(warning.contains("non-empty"));
        assert!(root.join("existing.txt").is_file());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INIT-COMMAND-008"))]
    fn force_overwrites_colliding_files_and_preserves_unrelated_entries() {
        let root = temp_root("force");
        std::fs::create_dir_all(&root).expect("target");
        std::fs::write(root.join("Cargo.toml"), "stale").expect("stale manifest");
        std::fs::write(root.join("keep.txt"), "keep").expect("unrelated file");
        let mut lock = FakeLock::default();
        let report = run(
            &request(&root, None, true),
            &mut lock,
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .expect("force init");
        assert_eq!(report.template, "basic");
        let manifest = std::fs::read_to_string(root.join("Cargo.toml")).expect("manifest");
        assert!(manifest.contains("[workspace]"));
        assert!(root.join("keep.txt").is_file());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INIT-LOCK-005"))]
    fn missing_tools_fail_before_writing() {
        let root = temp_root("missing-tools");
        let mut lock = FakeLock {
            missing: vec!["cargo"],
            ..FakeLock::default()
        };
        let error = run(
            &request(&root, None, false),
            &mut lock,
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .expect_err("missing tool");
        assert_eq!(error.exit_code(), 1);
        assert!(!lock.locked);
        assert!(!root.exists());
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INIT-LOCK-006"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-LOCK-007"))]
    fn lock_failure_preserves_scaffolded_files() {
        let root = temp_root("lock-failure");
        let mut lock = FakeLock {
            fail: true,
            ..FakeLock::default()
        };
        let error = run(
            &request(&root, None, false),
            &mut lock,
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .expect_err("lock failure");
        assert_eq!(error.exit_code(), 1);
        assert!(root.join("Cargo.toml").is_file());
        assert!(root.join("flake.nix").is_file());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-001"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-002"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-003"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-004"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-005"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-006"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-007"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-008"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-009"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-010"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-011"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-012"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-013"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-TEMPLATES-004"))]
    fn every_template_carries_the_common_scaffolding() {
        assert_eq!(TEMPLATE_NAMES, &["basic", "axum", "topcoat"]);
        for name in TEMPLATE_NAMES {
            let files = template_files(name);
            let flake = files.get("flake.nix").expect("flake");
            assert!(flake.contains("bloomery.mkFlake"), "{name} flake");
            assert!(
                flake.contains("github:actionable-work/bloomery"),
                "{name} bloomery input"
            );
            assert!(flake.contains("nixpkgs.url"), "{name} nixpkgs input");
            let manifest = files.get("Cargo.toml").expect("manifest");
            assert!(manifest.contains("[workspace]"), "{name} workspace");
            assert!(manifest.contains("resolver = \"2\""), "{name} resolver");
            let config = files.get(".bloomery/config.toml").expect("config");
            assert!(config.contains("[build]"), "{name} build table");
            assert!(
                config.contains("workspaceDependencies = false"),
                "{name} workspace dependency opt-out"
            );
            assert!(
                config.contains("noDefaultFeatures = false"),
                "{name} default feature opt-out"
            );
            assert!(files.contains_key(".gitignore"), "{name} gitignore");
            let gitignore = files.get(".gitignore").expect("gitignore");
            for entry in ["target/", "result", "result-*", ".direnv/"] {
                assert!(gitignore.contains(entry), "{name} gitignore {entry}");
            }
            assert!(
                files
                    .get(".envrc")
                    .is_some_and(|envrc| envrc.contains("use flake")),
                "{name} direnv"
            );
            assert!(manifest.contains("crates/"), "{name} multi-crate");
            assert!(
                files.keys().any(|path| path.ends_with("src/lib.rs")),
                "{name} library crate"
            );
            assert!(
                files.keys().any(|path| path.ends_with("src/main.rs")),
                "{name} binary crate"
            );
            let main = files
                .iter()
                .find(|(path, _)| path.ends_with("src/main.rs"))
                .map(|(_, content)| *content)
                .expect("main");
            assert!(main.matches("fn ").count() <= 1, "{name} thin binary");
            assert!(
                files
                    .keys()
                    .any(|path| path.starts_with(".bloomery/specs/") && path.ends_with(".toml")),
                "{name} requirement file"
            );
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-021"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-022"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-027"))]
    fn every_template_seeds_automated_specs_with_tagged_tests() {
        for name in TEMPLATE_NAMES {
            let files = template_files(name);
            let requirements: Vec<_> = files
                .iter()
                .filter(|(path, _)| path.ends_with(".toml") && path.contains("/requirements/"))
                .collect();
            assert!(!requirements.is_empty(), "{name} requirements");
            for (path, content) in &requirements {
                assert!(content.contains("manual = false"), "{name} {path}");
            }
            let libraries: Vec<_> = files
                .iter()
                .filter(|(path, content)| {
                    path.ends_with("src/lib.rs") && content.contains("cfg_attr(any(), bloomery(")
                })
                .collect();
            assert!(!libraries.is_empty(), "{name} tagged library test");
        }
    }

    // --- SyncLockStage integration with a fake tool runner ---

    struct FakeRunner;

    impl CommandRunner for FakeRunner {
        fn is_available(&self, _executable: &str) -> bool {
            true
        }

        fn run(
            &mut self,
            executable: &OsStr,
            arguments: &[OsString],
            working_directory: &Path,
        ) -> io::Result<CommandOutput> {
            let name = executable.to_string_lossy();
            let args: Vec<String> = arguments
                .iter()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect();
            if name == "nix" {
                std::fs::write(working_directory.join("flake.lock"), "{}\n")?;
            }
            if name == "cargo" {
                if args.first().map(String::as_str) == Some("update") {
                    std::fs::write(working_directory.join("Cargo.lock"), "version = 4\n")?;
                }
                if args.first().map(String::as_str) == Some("metadata") {
                    std::fs::write(working_directory.join("Cargo.lock"), "version = 4\n")?;
                    return Ok(CommandOutput {
                        code: Some(0),
                        stdout: metadata_json().into_bytes(),
                        stderr: Vec::new(),
                    });
                }
            }
            Ok(CommandOutput {
                code: Some(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
    }

    fn metadata_json() -> String {
        r#"{
  "packages": [
    {
      "id": "app 0.1.0 (path+file:///tmp/app)",
      "name": "app",
      "version": "0.1.0",
      "edition": "2024",
      "targets": [{ "kind": ["bin"] }]
    }
  ],
  "resolve": {
    "nodes": [
      {
        "id": "app 0.1.0 (path+file:///tmp/app)",
        "features": [],
        "deps": []
      }
    ]
  }
}"#
        .to_owned()
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INIT-LOCK-001"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-LOCK-002"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-LOCK-003"))]
    fn sync_lock_stage_creates_all_lockfiles() {
        let root = temp_root("sync-lock");
        let mut stage = SyncLockStage { runner: FakeRunner };
        let report = run(
            &request(&root, None, false),
            &mut stage,
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .expect("init with sync lock stage");
        assert!(root.join("Cargo.lock").is_file());
        assert!(root.join("bloomery.lock").is_file());
        assert!(root.join("flake.lock").is_file());
        assert_eq!(
            report.lockfiles,
            vec![
                "Cargo.lock".to_owned(),
                "bloomery.lock".to_owned(),
                "flake.lock".to_owned(),
            ]
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
