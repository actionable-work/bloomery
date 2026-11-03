use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct TemporaryWorkspace(PathBuf);

impl TemporaryWorkspace {
    fn new(template: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "bloomery-template-{template}-{}-{nonce}",
            std::process::id()
        ));
        assert!(
            !path.exists(),
            "temporary path already exists: {}",
            path.display()
        );
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_success(stage: &str, template: &str, output: Output) {
    assert!(
        output.status.success(),
        "{stage} failed for {template} with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires host Nix access and network; run with `nix run .#template-checks`"]
#[cfg_attr(any(), bloomery("CLI-INIT-TEMPLATES-012"))]
#[cfg_attr(any(), bloomery("CLI-INIT-FILES-029"))]
#[cfg_attr(any(), bloomery("CLI-INIT-FILES-031"))]
fn initialized_templates_pass_bloomery_check() {
    let bloomery = PathBuf::from(
        std::env::var("CARGO_BIN_EXE_bloomery")
            .expect("Cargo provides the bloomery binary path to integration tests"),
    );

    for template in ["basic", "axum", "topcoat"] {
        let workspace = TemporaryWorkspace::new(template);
        let mut init = Command::new(&bloomery);
        init.arg("init").arg(workspace.path());
        if template != "basic" {
            init.arg("--template").arg(template);
        }
        let output = init.output().expect("run bloomery init");
        assert_success("bloomery init", template, output);

        for lockfile in ["Cargo.lock", "bloomery.lock", "flake.lock"] {
            assert!(
                workspace.path().join(lockfile).is_file(),
                "{template} initialization did not create {lockfile}"
            );
        }

        let output = Command::new(&bloomery)
            .arg("check")
            .current_dir(workspace.path())
            .output()
            .expect("run bloomery check");
        assert_success("bloomery check", template, output);
        assert!(
            workspace
                .path()
                .join(".bloomery/code-health-summary.json")
                .is_file(),
            "the first write-mode check did not initialize {template}'s health baseline"
        );
    }
}
