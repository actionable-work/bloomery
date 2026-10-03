#![allow(clippy::result_large_err)]

mod loader;
mod markdown;

pub use loader::load;

#[cfg(test)]
mod tests {
    use super::load;
    use bloomery_test_macros::bloomery;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("bloomery-workspace-{suffix}"));
        let feature = root.join(".bloomery/specs/PARSER/WORKSPACE");
        fs::create_dir_all(feature.join("design")).expect("design");
        fs::create_dir_all(feature.join("requirements")).expect("requirements");
        fs::write(
            root.join(".bloomery/config.toml"),
            "[specs]\ndir = \"specs\"\n",
        )
        .expect("config");
        fs::write(
            root.join(".bloomery/specs/PARSER/README.md"),
            "---\nid: PARSER\nname: PARSER\ntagline: PARSER\ndescription: Service\n---\n# PARSER\n",
        )
        .expect("service README");
        fs::write(
            feature.join("README.md"),
            "---\nid: WORKSPACE\nname: Workspace\ntagline: Workspace\ndescription: Feature\n---\n# Workspace\n",
        )
        .expect("feature README");
        fs::write(feature.join("design/overview.md"), "# Overview\n").expect("design");
        fs::write(
            feature.join("requirements/STRUCTURE.toml"),
            "group = \"STRUCTURE\"\n\n[[requirements]]\nid = \"PARSER-WORKSPACE-STRUCTURE-001\"\ntitle = \"Structure\"\nmanual = true\n\n[requirements.ears]\ntype = \"ubiquitous\"\nsystem = \"workspace\"\naction = \"retain structure\"\n",
        )
        .expect("requirements");
        root
    }

    #[test]
    fn loads_a_complete_workspace() {
        let root = fixture();
        let context = load(&root).expect("workspace should load");
        assert_eq!(context.services.len(), 1);
        assert_eq!(context.requirements().count(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[bloomery("PARSER-CONFIGURATION-OPTIONAL-004")]
    fn loads_a_valid_workspace_without_a_configuration_file() {
        let root = fixture();
        fs::remove_file(root.join(".bloomery/config.toml")).expect("remove config");

        let context = load(&root).expect("workspace should load without configuration");
        assert_eq!(context.services.len(), 1);
        assert_eq!(context.requirements().count(), 1);
        assert_eq!(context.config.specs.dir, "specs");
        assert!(!context.config.scanners.rust.enabled);
        assert!(!context.config.scanners.playwright.enabled);
        assert!(!context.config.scanners.nix.enabled);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_a_feature_without_requirement_groups() {
        let root = fixture();
        fs::remove_file(root.join(".bloomery/specs/PARSER/WORKSPACE/requirements/STRUCTURE.toml"))
            .expect("requirements");
        let diagnostics = load(&root).expect_err("workspace should fail");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "MissingRequirementGroup")
        );
        let _ = fs::remove_dir_all(root);
    }
}
