use super::catalog::{
    BatchRealization, CheckPlan, FailureBlock, NixBackend, NixTaskResult, PlanError, PlannedSystem,
    nix_log_store_path,
};
use super::command::{CheckArgs, CheckContext, run_at_with};
use super::interrupt::{CancellationToken, InterruptFlag};
use super::progress::{DerivationEvent, ProgressSink};
use crate::output::TerminalFacts;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

static FIXTURE: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
pub(super) struct FakeNix {
    pub(super) systems: Mutex<Vec<String>>,
    pub(super) evaluations: AtomicUsize,
    pub(super) realizations: AtomicUsize,
    pub(super) build_commands: AtomicUsize,
    pub(super) built_derivations: Mutex<std::collections::BTreeSet<String>>,
    pub(super) names: Vec<String>,
    pub(super) discovery_error: Option<String>,
    pub(super) unique_derivations: bool,
    /// Attributes whose derivation fails during `plan_checks` evaluation.
    pub(super) plan_eval_failures: Vec<String>,
    /// Attributes whose derivation fails during batched realization. When empty
    /// and `realization_result` is a failure, every derivation fails.
    pub(super) failing_attributes: Vec<String>,
    /// `(dependent attribute, dependency attribute)` edges used for causal
    /// dependency-failure attribution.
    pub(super) derivation_dependencies: Vec<(String, String)>,
    pub(super) validity_queries: AtomicUsize,
    pub(super) valid_outputs: Mutex<std::collections::BTreeSet<String>>,
    pub(super) interrupt_on_evaluation: Option<InterruptFlag>,
    pub(super) force_manifest_failure: bool,
    pub(super) wait_for_cancellation: bool,
    pub(super) realization_result: Option<NixTaskResult<()>>,
    pub(super) log_contents: Option<Vec<u8>>,
    pub(super) nix_logs: Mutex<std::collections::BTreeMap<String, Result<Vec<u8>, String>>>,
    pub(super) nix_log_requests: AtomicUsize,
    pub(super) active: AtomicUsize,
    pub(super) maximum_active: AtomicUsize,
    /// Derivation events emitted for every realization request. Tests use this
    /// to exercise progress wiring without a real Nix daemon.
    pub(super) scripted_events: Mutex<Vec<DerivationEvent>>,
    /// Ordered record of formatter and realization activity so tests can assert
    /// that the formatter gate precedes every selected check.
    pub(super) events: Mutex<Vec<String>>,
    /// Number of times the formatter gate ran.
    pub(super) formatter_runs: AtomicUsize,
    /// Optional formatter result; defaults to success.
    pub(super) formatter_result: Option<NixTaskResult<()>>,
    /// Optional formatter log contents.
    pub(super) formatter_log_contents: Option<Vec<u8>>,
    /// Files the fake formatter writes relative to the workspace root. Applied
    /// regardless of the result so partial edits can be tested.
    pub(super) formatter_edits: Vec<(String, Vec<u8>)>,
    /// Files the fake formatter removes relative to the workspace root.
    pub(super) formatter_removals: Vec<String>,
    /// Events emitted only when the realized attribute matches the key.
    pub(super) events_by_attribute: Mutex<BTreeMap<String, Vec<DerivationEvent>>>,
    /// When set, realize_check blocks until the test releases this barrier so a
    /// run can be observed while one Nix task is still active.
    pub(super) hold_until_released: Option<std::sync::Arc<std::sync::Barrier>>,
    /// When set, realize_check blocks until the run is canceled so tests can
    /// observe a completed check while another task is still active.
    pub(super) complete_after_cancellation: bool,
}

impl NixBackend for FakeNix {
    fn host_system(&self, _root: &Path) -> Result<String, String> {
        Ok("x86_64-linux".to_owned())
    }

    fn discover(&self, _root: &Path, system: &str) -> Result<Vec<String>, String> {
        self.systems
            .lock()
            .expect("systems lock")
            .push(system.to_owned());
        if let Some(error) = &self.discovery_error {
            return Err(error.clone());
        }
        Ok(self.names.clone())
    }

    fn format_workspace(
        &self,
        root: &Path,
        log_path: &Path,
        _cancellation: CancellationToken,
    ) -> NixTaskResult<()> {
        self.formatter_runs.fetch_add(1, Ordering::SeqCst);
        self.events
            .lock()
            .expect("event lock")
            .push("format".to_owned());
        let contents = self
            .formatter_log_contents
            .clone()
            .unwrap_or_else(|| b"formatter output\n".to_vec());
        fs::write(log_path, contents).expect("formatter log");
        for (path, contents) in &self.formatter_edits {
            fs::write(root.join(path), contents).expect("formatter edit");
        }
        for path in &self.formatter_removals {
            let _ = fs::remove_file(root.join(path));
        }
        self.formatter_result
            .clone()
            .unwrap_or(NixTaskResult::Succeeded(()))
    }

    fn plan_checks(&self, _root: &Path, system: &str) -> Result<PlannedSystem, String> {
        self.systems
            .lock()
            .expect("systems lock")
            .push(system.to_owned());
        self.evaluations.fetch_add(1, Ordering::SeqCst);
        if let Some(error) = &self.discovery_error {
            return Err(error.clone());
        }
        let mut plans = Vec::new();
        for attribute in &self.names {
            let derivation = if self.unique_derivations {
                format!("/nix/store/{attribute}.drv")
            } else {
                "/nix/store/check.drv".to_owned()
            };
            if self.plan_eval_failures.contains(attribute) {
                plans.push(CheckPlan {
                    attribute: attribute.clone(),
                    derivation: None,
                    eval_error: Some(PlanError {
                        code: "EvaluationFailed".to_owned(),
                        message: format!("attribute evaluation failed for {attribute}"),
                    }),
                    outputs: Vec::new(),
                });
                continue;
            }
            let output = if self.unique_derivations {
                format!("/nix/store/{attribute}-out")
            } else {
                "/nix/store/check-out".to_owned()
            };
            plans.push(CheckPlan {
                attribute: attribute.clone(),
                derivation: Some(derivation),
                eval_error: None,
                outputs: vec![output],
            });
        }
        let mut dependencies = BTreeMap::new();
        for (dependent, dependency) in &self.derivation_dependencies {
            let dependent_drv = if self.unique_derivations {
                format!("/nix/store/{dependent}.drv")
            } else {
                "/nix/store/check.drv".to_owned()
            };
            let dependency_drv = if self.unique_derivations {
                format!("/nix/store/{dependency}.drv")
            } else {
                "/nix/store/check.drv".to_owned()
            };
            dependencies.insert(dependent_drv, vec![dependency_drv]);
        }
        Ok(PlannedSystem {
            legacy: false,
            plans,
            dependencies,
        })
    }

    fn realize_batch(
        &self,
        _root: &Path,
        plans: &[CheckPlan],
        _fail_fast: bool,
        log_path: &Path,
        cancellation: CancellationToken,
        progress: &ProgressSink,
    ) -> BatchRealization {
        self.build_commands.fetch_add(1, Ordering::SeqCst);
        self.events
            .lock()
            .expect("event lock")
            .push("batch".to_owned());
        let contents = self.log_contents.clone().unwrap_or_else(|| {
            plans
                .iter()
                .map(|plan| format!("build log output for {}\n", plan.attribute))
                .collect::<String>()
                .into_bytes()
        });
        fs::write(log_path, &contents).expect("batch log");
        if self.force_manifest_failure {
            let run_directory = log_path.parent().unwrap().parent().unwrap();
            fs::create_dir(run_directory.join("manifest.json"))
                .expect("preempt manifest publication");
        }
        let mut build_started = Vec::new();
        for plan in plans {
            let emitted = self
                .events_by_attribute
                .lock()
                .expect("scripted event lock")
                .get(&plan.attribute)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .chain(
                    self.scripted_events
                        .lock()
                        .expect("scripted event lock")
                        .clone(),
                )
                .collect::<Vec<_>>();
            for event in &emitted {
                if let DerivationEvent::BuildStarted { drv } = event {
                    build_started.push(drv.clone());
                }
                progress.work(event.clone());
            }
        }
        if let Some(interrupt) = &self.interrupt_on_evaluation {
            interrupt.interrupt_for_test();
        }
        let all_fail = matches!(self.realization_result, Some(NixTaskResult::Failed { .. }));
        let mut failures = BTreeMap::new();
        let mut valid = std::collections::BTreeSet::new();
        let mut new_realizations = 0usize;
        let hint = nix_log_store_path(&contents);
        for plan in plans {
            let Some(derivation) = &plan.derivation else {
                continue;
            };
            let first = self
                .built_derivations
                .lock()
                .expect("derivation lock")
                .insert(derivation.clone());
            if first {
                new_realizations += 1;
                let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
                self.maximum_active.fetch_max(active, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(10));
                self.active.fetch_sub(1, Ordering::SeqCst);
            }
            let fails = if !self.failing_attributes.is_empty() {
                self.failing_attributes.contains(&plan.attribute)
            } else {
                all_fail
            };
            let dependency_failed =
                self.derivation_dependencies
                    .iter()
                    .any(|(dependent, dependency)| {
                        dependent == &plan.attribute && self.failing_attributes.contains(dependency)
                    });
            if fails {
                let (code, message) = match &self.realization_result {
                    Some(NixTaskResult::Failed { code, message }) => {
                        (code.clone(), message.clone())
                    }
                    _ => ("NixCheckFailed".to_owned(), "build failed".to_owned()),
                };
                failures.insert(
                    derivation.clone(),
                    FailureBlock {
                        derivation: derivation.clone(),
                        code,
                        message,
                        log_hint: hint.clone().or_else(|| Some(derivation.clone())),
                        excerpt: Vec::new(),
                    },
                );
                if _fail_fast {
                    break;
                }
            } else if !dependency_failed {
                valid.extend(plan.outputs.iter().cloned());
            }
        }
        self.realizations
            .fetch_add(new_realizations, Ordering::SeqCst);
        if self.wait_for_cancellation || self.complete_after_cancellation {
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while !cancellation.is_canceled() && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        if let Some(barrier) = &self.hold_until_released {
            barrier.wait();
        }
        let interrupted = self
            .interrupt_on_evaluation
            .as_ref()
            .is_some_and(|interrupt| interrupt.is_set());
        if cancellation.is_canceled() || interrupted {
            *self.valid_outputs.lock().expect("valid outputs lock") =
                std::collections::BTreeSet::new();
            return BatchRealization {
                failures: BTreeMap::new(),
                canceled: true,
                operational_error: None,
            };
        }
        *self.valid_outputs.lock().expect("valid outputs lock") = valid.clone();
        if failures.is_empty() {
            if build_started.is_empty() {
                progress.emit(super::progress::ProgressEvent::DerivationMetadataAvailable);
            } else {
                for drv in &build_started {
                    progress.work(DerivationEvent::BuildSucceeded { drv: drv.clone() });
                }
            }
        }
        BatchRealization {
            failures,
            canceled: false,
            operational_error: None,
        }
    }

    fn validate_outputs(
        &self,
        _root: &Path,
        outputs: &[String],
    ) -> Result<std::collections::BTreeSet<String>, String> {
        self.validity_queries.fetch_add(1, Ordering::SeqCst);
        let valid = self
            .valid_outputs
            .lock()
            .expect("valid outputs lock")
            .clone();
        Ok(outputs
            .iter()
            .filter(|output| valid.contains(*output))
            .cloned()
            .collect())
    }

    fn read_derivation_log(&self, _root: &Path, store_path: &str) -> Result<Vec<u8>, String> {
        self.nix_log_requests.fetch_add(1, Ordering::SeqCst);
        self.nix_logs
            .lock()
            .expect("Nix log lock")
            .get(store_path)
            .cloned()
            .unwrap_or_else(|| Err(format!("no fake log configured for {store_path}")))
    }
}

pub(super) fn fixture(with_flake: bool) -> (PathBuf, PathBuf) {
    let id = FIXTURE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "bloomery-check-command-{}-{id}",
        std::process::id()
    ));
    let cache = std::env::temp_dir().join(format!(
        "bloomery-check-command-cache-{}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("workspace root");
    if with_flake {
        fs::write(root.join("flake.nix"), "{}\n").expect("flake marker");
    }
    fs::create_dir_all(root.join(".bloomery")).expect("configuration directory");
    fs::write(
        root.join(".bloomery/config.toml"),
        "[specs]\ndir = \"specs\"\n",
    )
    .expect("configuration");
    let feature = root.join(".bloomery/specs/CLI/CHECK");
    fs::create_dir_all(feature.join("design")).expect("design directory");
    fs::create_dir_all(feature.join("requirements")).expect("requirements directory");
    fs::write(
        root.join(".bloomery/specs/CLI/README.md"),
        "---\nid: CLI\nname: CLI\ntagline: CLI\ndescription: Area\n---\n# CLI\n",
    )
    .expect("area README");
    fs::write(
        feature.join("README.md"),
        "---\nid: CHECK\nname: Check\ntagline: Check\ndescription: Feature\n---\n# Check\n",
    )
    .expect("feature README");
    fs::write(feature.join("design/pipeline.md"), "# Pipeline\n").expect("design file");
    fs::write(
        feature.join("requirements/CONTRACT.toml"),
        "group = \"CONTRACT\"\n\n[[requirements]]\nid = \"CLI-CHECK-CONTRACT-001\"\ntitle = \"Fixture\"\nmanual = true\n\n[requirements.ears]\ntype = \"ubiquitous\"\nsystem = \"fixture\"\naction = \"exercise\"\n",
    )
    .expect("requirements");
    (root, cache)
}

pub(super) fn request() -> CheckArgs {
    CheckArgs::default()
}

pub(super) fn nix_is_available() -> bool {
    Command::new("nix")
        .args(["store", "ping", "--json"])
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Repository root, used to reference the checked-in `flake.lock` when locking
/// real-Nix fixture flakes offline.
pub(super) fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../..")
}

/// Generate a `flake.lock` for a real-Nix fixture flake using the repository's
/// locked nixpkgs, so `nix fmt` can build the fixture formatter offline.
pub(super) fn lock_fixture_flake(root: &Path) {
    let reference = repository_root().join("flake.lock");
    let status = Command::new("nix")
        .args(["flake", "lock", "--reference-lock-file"])
        .arg(&reference)
        .current_dir(root)
        .status()
        .expect("run nix flake lock");
    assert!(
        status.success(),
        "nix flake lock failed for {}",
        root.display()
    );
}

pub(super) fn invoke(
    args: CheckArgs,
    root: &Path,
    cache: &Path,
    backend: &dyn NixBackend,
    json: bool,
) -> (std::process::ExitCode, Vec<u8>, Vec<u8>) {
    invoke_with_interrupt(args, root, cache, backend, json, InterruptFlag::for_test())
}

pub(super) fn invoke_with_interrupt(
    args: CheckArgs,
    root: &Path,
    cache: &Path,
    backend: &dyn NixBackend,
    json: bool,
    interrupt: InterruptFlag,
) -> (std::process::ExitCode, Vec<u8>, Vec<u8>) {
    invoke_with_terminal(
        args,
        root,
        cache,
        backend,
        json,
        interrupt,
        TerminalFacts::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn invoke_with_terminal(
    args: CheckArgs,
    root: &Path,
    cache: &Path,
    backend: &dyn NixBackend,
    json: bool,
    interrupt: InterruptFlag,
    terminal: TerminalFacts,
) -> (std::process::ExitCode, Vec<u8>, Vec<u8>) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = run_at_with(
        args,
        CheckContext {
            root,
            json_mode: json,
            backend,
            interrupt,
            cache_base: Some(cache),
            terminal,
        },
        &mut stdout,
        &mut stderr,
    );
    (code, stdout, stderr)
}
