use super::super::catalog::NixCli;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) struct NixFixture {
    workspace: PathBuf,
    root: PathBuf,
    calls: Vec<PathBuf>,
}

impl NixFixture {
    /// Call records live beside the workspace, matching the fixture
    /// executable, so checks that scan the workspace never observe them.
    pub(crate) fn new(workspace: &Path) -> Self {
        let workspace = fs::canonicalize(workspace).expect("fixture workspace");
        let name = workspace.file_name().expect("fixture workspace name");
        let root = workspace.with_file_name(format!("{}.nix-fixture", name.to_string_lossy()));
        fs::create_dir(&root).expect("fixture script directory");
        Self {
            workspace,
            root,
            calls: Vec::new(),
        }
    }

    pub(crate) fn executable() -> PathBuf {
        let path = std::env::var_os("BLOOMERY_NIX_FIXTURE")
            .expect("BLOOMERY_NIX_FIXTURE must name the declared test fixture executable");
        let path = PathBuf::from(path);
        assert!(
            path.is_absolute() && path.is_file(),
            "missing fixture executable: {}",
            path.display()
        );
        path
    }

    pub(crate) fn backend(&self) -> NixCli {
        let path = Self::executable();
        NixCli::with_executables(path.clone(), path)
    }

    pub(crate) fn expect(&mut self, args: &[String], stdout: &str) -> PathBuf {
        let call = self.root.join(format!("{:04}", self.calls.len()));
        fs::create_dir(&call).expect("fixture call");
        let encoded = args
            .iter()
            .flat_map(|arg| arg.bytes().chain([0]))
            .collect::<Vec<_>>();
        fs::write(call.join("args"), encoded).unwrap();
        fs::write(call.join("cwd"), self.workspace.to_str().unwrap()).unwrap();
        fs::write(call.join("stdout"), stdout).unwrap();
        fs::write(call.join("stderr"), "").unwrap();
        fs::write(call.join("status"), "0").unwrap();
        self.calls.push(call.clone());
        call
    }

    pub(crate) fn verify(&self) -> Result<(), String> {
        for entry in fs::read_dir(&self.root).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("error-")
            {
                return Err(fs::read_to_string(path).unwrap());
            }
        }
        for call in &self.calls {
            if !call.join("finished").exists() && !call.join("blocked").exists() {
                return Err(format!(
                    "expected fixture call did not finish: {}",
                    call.display()
                ));
            }
        }
        Ok(())
    }
}
