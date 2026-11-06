use super::*;
use crate::source::MemoryTree;

const DECLARATION: &str =
    "[[test]]\nname='sample'\npath='tests/a.rs'\nrequired-features=['fast', 'fast', 'slow']";

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-TARGET-001"))]
fn projects_declared_fields_and_rejects_invalid_declarations() {
    let tree = MemoryTree::new([
        ("Cargo.toml", DECLARATION),
        ("tests/a.rs", "#[test] fn a() {}"),
    ])
    .unwrap();
    let target = load_test_target(&tree, "Cargo.toml", "sample").unwrap();
    assert_eq!(target.path, "tests/a.rs");
    assert_eq!(
        target.required_features,
        BTreeSet::from(["fast".into(), "slow".into()])
    );
    assert!(target.harness);
    for (document, code) in [
        ("broken [".to_owned(), "CargoTargetManifestError"),
        (
            "[package]\nname='example'".into(),
            "CargoTestTargetNotFound",
        ),
        (
            DECLARATION.replace("name='sample'", "name='other'"),
            "CargoTestTargetNotFound",
        ),
        (
            DECLARATION.replace("path='tests/a.rs'", ""),
            "UnsupportedCargoTestTarget",
        ),
        (
            DECLARATION.replace("['fast', 'fast', 'slow']", "'fast'"),
            "CargoTargetManifestError",
        ),
        (
            DECLARATION.replace("['fast', 'fast', 'slow']", "[1]"),
            "CargoTargetManifestError",
        ),
        (
            format!("{DECLARATION}\nharness='true'"),
            "CargoTargetManifestError",
        ),
        ("[[test]]\nname=1".into(), "CargoTargetManifestError"),
    ] {
        let tree =
            MemoryTree::new([("Cargo.toml", document.as_str()), ("tests/a.rs", "")]).unwrap();
        let error = load_test_target(&tree, "Cargo.toml", "sample").unwrap_err();
        assert_eq!(error.code, code);
        assert_eq!(error.location.unwrap().path.to_str(), Some("Cargo.toml"));
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-TARGET-002"))]
fn validates_paths_collisions_and_available_content() {
    let nested = DECLARATION.replace("tests/a.rs", "../shared/./a.rs");
    let tree = MemoryTree::new([("pkg/Cargo.toml", nested.as_str()), ("shared/a.rs", "")]).unwrap();
    assert_eq!(
        load_test_target(&tree, "pkg/Cargo.toml", "sample")
            .unwrap()
            .path,
        "shared/a.rs"
    );
    for path in [
        "../a.rs",
        "/tests/a.rs",
        "tests/../../a.rs",
        "C:\\a.rs",
        "",
        "absent.rs",
    ] {
        let document = DECLARATION.replace("tests/a.rs", path);
        let tree = MemoryTree::new([("Cargo.toml", document.as_str())]).unwrap();
        assert_eq!(
            load_test_target(&tree, "Cargo.toml", "sample")
                .unwrap_err()
                .code,
            "CargoTargetPathError",
            "{path}"
        );
    }
    let duplicate = format!("{DECLARATION}\n{DECLARATION}");
    let tree = MemoryTree::new([("Cargo.toml", duplicate)]).unwrap();
    assert_eq!(
        load_test_target(&tree, "Cargo.toml", "sample")
            .unwrap_err()
            .code,
        "DuplicateCargoTestTarget"
    );
    assert_eq!(
        load_test_target(&tree, "missing/Cargo.toml", "sample")
            .unwrap_err()
            .code,
        "CargoTargetReadError"
    );
    assert_eq!(
        load_test_target(&Unavailable, "Cargo.toml", "sample")
            .unwrap_err()
            .code,
        "CargoTargetReadError"
    );
    assert_eq!(
        load_test_target(&Collision, "Cargo.toml", "sample")
            .unwrap_err()
            .code,
        "SourceTreeError"
    );
    for manifest in [
        "../Cargo.toml",
        "/Cargo.toml",
        "pkg/./Cargo.toml",
        "other.toml",
    ] {
        assert!(validate_selector(manifest, "sample").is_err());
    }
    assert!(validate_selector("Cargo.toml", " ").is_err());
}

struct Unavailable;
impl SourceTree for Unavailable {
    fn paths(&self) -> Result<Vec<String>, String> {
        Ok(vec!["Cargo.toml".into(), "tests/a.rs".into()])
    }
    fn read(&self, path: &str) -> Result<String, String> {
        if path == "Cargo.toml" {
            Ok(DECLARATION.into())
        } else {
            Err("unavailable content".into())
        }
    }
}
struct Collision;
impl SourceTree for Collision {
    fn paths(&self) -> Result<Vec<String>, String> {
        Ok(vec![
            "Cargo.toml".into(),
            "tests".into(),
            "tests/a.rs".into(),
        ])
    }
    fn read(&self, _: &str) -> Result<String, String> {
        Ok(DECLARATION.into())
    }
}
