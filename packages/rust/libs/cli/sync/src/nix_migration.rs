#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    fn repository_root() -> PathBuf {
        let configured_root = std::env::var_os("BLOOMERY_SOURCE_ROOT").map(PathBuf::from);
        if let Some(root) = configured_root.filter(|root| root.join("lib/default.nix").is_file()) {
            return root;
        }
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .find(|candidate| candidate.join("lib/default.nix").is_file())
            .map(Path::to_path_buf)
            .expect("repository Nix sources must be available to migration tests")
    }

    fn source(root: &Path, path: &str) -> String {
        fs::read_to_string(root.join(path))
            .unwrap_or_else(|error| panic!("unable to read {path}: {error}"))
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-INTERFACE-004"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-INTERFACE-017"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-INTERFACE-018"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-INTERFACE-019"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-INTERFACE-020"))]
    fn nix_workspace_apis_remove_lock_scripts_and_preserve_regular_apps() {
        let root = repository_root();
        let api = source(&root, "lib/default.nix");
        let workspace = source(&root, "lib/mk-workspace.nix");
        let flake = source(&root, "lib/mk-flake.nix");
        let module = source(&root, "lib/modules/flake-module.nix");

        assert!(!root.join("lib/lock/default.nix").exists());
        assert!(!api.contains("lockScript"));
        assert!(!api.contains("lockApp"));
        assert!(!api.contains("apps = {"));
        assert!(!workspace.contains("lockTools"));
        assert!(!workspace.contains("lockApp"));
        assert!(!workspace.contains("lockScript"));
        assert!(!workspace.contains("lock = lockApp"));
        assert!(!flake.contains("lockApp"));
        assert!(!module.contains("lockApp"));
        assert!(workspace.contains("binApps"));
        assert!(workspace.contains("docApps"));
        assert!(workspace.contains("default = binApps.default or firstBin"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-INTERFACE-021"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-INTERFACE-022"))]
    fn nix_lock_validation_is_read_only_and_points_repairs_to_sync() {
        let root = repository_root();
        let validator = source(&root, "lib/workspace/lock-check.nix");
        let workspace = source(&root, "lib/mk-workspace.nix");

        assert!(validator.contains("Run 'bloomery sync'"));
        assert!(!validator.contains("nix run <bloomery>#lock"));
        assert!(workspace.contains("Run 'bloomery sync'"));
        assert!(!workspace.contains("nix run <bloomery>#lock"));
        assert!(!validator.contains("writeFile"));
        assert!(!validator.contains("builtins.write"));
        assert!(!validator.contains("Command::"));
    }
}
