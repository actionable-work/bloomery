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
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn fixture() -> PathBuf {
        let suffix = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "bloomery-workspace-{}-{suffix}",
            std::process::id()
        ));
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
            "---\nid: PARSER\nname: PARSER\ntagline: PARSER\ndescription: Area\n---\n# PARSER\n",
        )
        .expect("area README");
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
        assert_eq!(context.areas.len(), 1);
        assert_eq!(context.requirements().count(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[bloomery("PARSER-CONFIGURATION-REQUIRED-001")]
    fn missing_configuration_prevents_workspace_loading() {
        let root = fixture();
        fs::remove_file(root.join(".bloomery/config.toml")).expect("remove config");

        let diagnostics = load(&root).expect_err("workspace should require configuration");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "ConfigurationError");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[bloomery("PARSER-WORKSPACE-STRUCTURE-003")]
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

    #[test]
    #[bloomery("PARSER-WORKSPACE-STRUCTURE-001")]
    fn rejects_a_feature_without_its_readme() {
        let root = fixture();
        fs::remove_file(root.join(".bloomery/specs/PARSER/WORKSPACE/README.md"))
            .expect("feature README");

        let diagnostics = load(&root).expect_err("feature README is required");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "MissingDocument")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[bloomery("PARSER-WORKSPACE-STRUCTURE-002")]
    fn rejects_a_feature_without_its_design_directory() {
        let root = fixture();
        fs::remove_dir_all(root.join(".bloomery/specs/PARSER/WORKSPACE/design"))
            .expect("design directory");

        let diagnostics = load(&root).expect_err("feature design directory is required");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "MissingDesignDirectory")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[bloomery("PARSER-REQUIREMENTS-FORMAT-001")]
    fn rejects_requirement_ids_that_do_not_match_their_contract() {
        let root = fixture();
        let path = root.join(".bloomery/specs/PARSER/WORKSPACE/requirements/STRUCTURE.toml");
        let contents = fs::read_to_string(&path).expect("requirement group");
        fs::write(
            &path,
            contents.replace(
                "PARSER-WORKSPACE-STRUCTURE-001",
                "PARSER-WORKSPACE-STRUCTURE-00x",
            ),
        )
        .expect("invalid ID");

        let diagnostics = load(&root).expect_err("invalid requirement ID must fail loading");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "InvalidSpecId")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[bloomery("PARSER-REQUIREMENTS-FORMAT-002")]
    fn rejects_ears_records_with_missing_or_unknown_fields() {
        let root = fixture();
        let path = root.join(".bloomery/specs/PARSER/WORKSPACE/requirements/STRUCTURE.toml");
        let contents = fs::read_to_string(&path).expect("requirement group");
        for malformed in [
            contents.replace("type = \"ubiquitous\"", "type = \"unknown\""),
            contents.replace("action = \"retain structure\"\n", ""),
        ] {
            fs::write(&path, malformed).expect("invalid EARS record");
            let diagnostics = load(&root).expect_err("invalid EARS record must fail loading");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == "ParseError")
            );
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[bloomery("PARSER-REQUIREMENTS-FORMAT-003")]
    fn resolves_design_paths_and_heading_anchors() {
        let root = fixture();
        let feature = root.join(".bloomery/specs/PARSER/WORKSPACE");
        fs::write(feature.join("design/overview.md"), "# Overview\n").expect("design document");
        let requirements = feature.join("requirements/STRUCTURE.toml");
        let contents = fs::read_to_string(&requirements).expect("requirement group");
        fs::write(
            &requirements,
            contents.replace(
                "title = \"Structure\"",
                "title = \"Structure\"\ndesign = \"design/overview.md#overview\"",
            ),
        )
        .expect("design reference");

        let context = load(&root).expect("existing design reference should resolve");
        let requirement = context.requirements().next().expect("requirement").2;
        assert_eq!(requirement.design.path, feature.join("design/overview.md"));
        assert_eq!(requirement.design.anchor.as_deref(), Some("overview"));
        assert_eq!(
            requirement.design.display,
            ".bloomery/specs/PARSER/WORKSPACE/design/overview.md"
        );
        let _ = fs::remove_dir_all(root);
    }
}
