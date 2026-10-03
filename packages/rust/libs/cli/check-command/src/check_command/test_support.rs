use super::catalog::{NixBackend, NixTaskResult};
use super::command::{CheckArgs, CheckContext, run_at_with};
use super::interrupt::{CancellationToken, InterruptFlag};
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
    pub(super) interrupt_on_evaluation: Option<InterruptFlag>,
    pub(super) force_manifest_failure: bool,
    pub(super) wait_for_cancellation: bool,
    pub(super) realization_result: Option<NixTaskResult<()>>,
    pub(super) log_contents: Option<Vec<u8>>,
    pub(super) nix_logs: Mutex<std::collections::BTreeMap<String, Result<Vec<u8>, String>>>,
    pub(super) nix_log_requests: AtomicUsize,
    pub(super) active: AtomicUsize,
    pub(super) maximum_active: AtomicUsize,
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

    fn realize_check(
        &self,
        _root: &Path,
        _system: &str,
        attribute: &str,
        log_path: &Path,
        cancellation: CancellationToken,
    ) -> NixTaskResult<()> {
        self.evaluations.fetch_add(1, Ordering::SeqCst);
        self.build_commands.fetch_add(1, Ordering::SeqCst);
        if cancellation.is_canceled() {
            return NixTaskResult::Canceled;
        }
        let derivation = if self.unique_derivations {
            format!("/nix/store/{attribute}.drv")
        } else {
            "/nix/store/check.drv".to_owned()
        };
        let log_contents = self
            .log_contents
            .clone()
            .unwrap_or_else(|| format!("build log output for {attribute}\n").into_bytes());
        fs::write(log_path, log_contents).expect("build log");
        if self.force_manifest_failure {
            let run_directory = log_path.parent().unwrap().parent().unwrap();
            fs::create_dir(run_directory.join("manifest.json"))
                .expect("preempt manifest publication");
        }
        if let Some(interrupt) = &self.interrupt_on_evaluation {
            interrupt.interrupt_for_test();
        }
        // Model Nix's output lock: aliases issue separate client requests,
        // but share one realization attempt.
        let first_realization = self
            .built_derivations
            .lock()
            .expect("derivation lock")
            .insert(derivation);
        if first_realization {
            self.realizations.fetch_add(1, Ordering::SeqCst);
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.maximum_active.fetch_max(active, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(10));
            self.active.fetch_sub(1, Ordering::SeqCst);
        }
        if self.wait_for_cancellation {
            // A bounded wait makes delayed traceability admission fail the test
            // instead of hanging the suite indefinitely.
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while !cancellation.is_canceled() && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        if cancellation.is_canceled() {
            return NixTaskResult::Canceled;
        }
        self.realization_result
            .clone()
            .unwrap_or(NixTaskResult::Succeeded(()))
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
        },
        &mut stdout,
        &mut stderr,
    );
    (code, stdout, stderr)
}
