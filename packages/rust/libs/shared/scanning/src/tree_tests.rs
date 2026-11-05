use crate::{
    DiscoveryState, FilesystemTree, MemoryTree, ScanScope, SourceTree, scan_all, scan_tree,
};
use bloomery_model::{Config, Context, Diagnostic, config::ScannersConfig};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(files: &[(&str, &str)]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "bloomery-tree-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("root");
        for (path, source) in files {
            let path = root.join(path);
            fs::create_dir_all(path.parent().expect("parent")).expect("directory");
            fs::write(path, source).expect("source");
        }
        Self(root)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove fixture");
    }
}

fn config() -> ScannersConfig {
    let mut config = ScannersConfig::default();
    config.rust.paths = vec!["tests/**/*.rs".into(), "tests/a.rs".into()];
    config.playwright.paths = vec!["e2e/**/*.ts".into()];
    config.nix.paths = vec!["**/*.nix".into()];
    config.nix.test_paths = vec!["**/*.test.nix".into()];
    config
}

const FILES: &[(&str, &str)] = &[
    (
        "tests/a.rs",
        "#[cfg(any())]\n#[cfg_attr(any(), bloomery(\"DEMO-TESTS-BIND-001\"))]\n#[test]\nfn tagged() { panic!(\"must not run\"); }\n#[test]\nfn untied() {}\n",
    ),
    (
        "e2e/a.ts",
        "test('tagged', { tag: ['@bloomery:DEMO-TESTS-BIND-001'] }, async () => { throw new Error('must not run'); });\ntest('untied', async () => {});\n",
    ),
    (
        "checks/a.test.nix",
        "{ passthru.bloomery = [ \"DEMO-TESTS-BIND-001\" ]; }\n",
    ),
    ("checks/plain.nix", "builtins.abort \"must not evaluate\"\n"),
    ("target/ignored.rs", "not Rust"),
];

fn diagnostics_key(
    errors: Vec<Diagnostic>,
) -> Vec<(
    String,
    String,
    Option<bloomery_model::SourceLocation>,
    Vec<String>,
)> {
    errors
        .into_iter()
        .map(|error| {
            (
                error.code,
                error.message,
                error.location.map(|location| *location),
                error.notes,
            )
        })
        .collect()
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-SCANNING-TREES-001"))]
#[cfg_attr(any(), bloomery("PARSER-SCANNING-TREES-006"))]
fn adapters_preserve_bindings_locations_scope_and_states() {
    let _guard = crate::tests::scan_lock();
    let disk = Fixture::new(FILES);
    let memory = MemoryTree::new(FILES.iter().copied()).expect("virtual tree");
    let virtual_result = scan_tree(&memory, &config()).expect("virtual scan");
    assert_eq!(
        virtual_result,
        scan_tree(&FilesystemTree::new(&disk.0), &config()).expect("filesystem scan")
    );
    assert_eq!(
        virtual_result,
        scan_tree(
            &MemoryTree::new(FILES.iter().rev().copied()).expect("reversed"),
            &config()
        )
        .expect("reversed scan")
    );
    assert_eq!(virtual_result.scope, ScanScope::Syntactic);
    assert!(virtual_result.is_complete());
    assert_eq!(virtual_result.output.evidence.len(), 3);
    assert_eq!(virtual_result.output.tests.len(), 5);
    assert!(
        virtual_result
            .output
            .evidence
            .iter()
            .all(|item| item.location.path.is_relative() && item.location.line.is_some())
    );
    let context = Context {
        specs_enabled: true,
        root: disk.0.clone(),
        config: Config {
            scanners: config(),
            ..Config::default()
        },
        areas: Vec::new(),
    };
    let mut legacy = scan_all(&context).expect("compatibility API");
    for evidence in &mut legacy.evidence {
        evidence.location.path = evidence
            .location
            .path
            .strip_prefix(&disk.0)
            .expect("relative")
            .into();
    }
    for test in &mut legacy.tests {
        test.location.path = test
            .location
            .path
            .strip_prefix(&disk.0)
            .expect("relative")
            .into();
    }
    assert_eq!(legacy, virtual_result.output);
    // The IDs remain unresolved literals until a specification checker runs.
    assert_eq!(
        legacy
            .tests
            .iter()
            .filter(|test| !test.references.is_empty())
            .count(),
        3
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-SCANNING-TREES-001"))]
#[cfg_attr(any(), bloomery("PARSER-SCANNING-TREES-006"))]
fn malformed_sources_have_identical_ordered_diagnostics() {
    let _guard = crate::tests::scan_lock();
    let files = [
        ("tests/a.rs", "fn broken( {"),
        (
            "tests/z.rs",
            "#[test]\n#[cfg_attr(any(), bloomery(123))]\nfn bad_tag() {}",
        ),
        ("e2e/b.ts", "test('unterminated'"),
        (
            "checks/b.test.nix",
            "{ passthru.bloomery = [ \"invalid\" ]; }",
        ),
    ];
    let disk = Fixture::new(&files);
    let memory = MemoryTree::new(files.iter().rev().copied()).expect("tree");
    let errors = diagnostics_key(scan_tree(&memory, &config()).expect_err("invalid source"));
    assert_eq!(
        errors,
        diagnostics_key(
            scan_tree(&FilesystemTree::new(&disk.0), &config()).expect_err("invalid source")
        )
    );
    assert_eq!(
        errors
            .iter()
            .map(|error| error.0.as_str())
            .collect::<Vec<_>>(),
        [
            "NixReferenceError",
            "PlaywrightParseError",
            "RustParseError",
            "RustReferenceError"
        ]
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-SCANNING-TREES-002"))]
fn rejects_ambiguous_paths_and_collisions() {
    for path in [
        "", "/a.rs", "../a.rs", "a/../b", "./a", "a//b", "a/", "C:/a", "a\\b", "a/./b", "a\0b",
    ] {
        assert!(MemoryTree::new([(path, "")]).is_err(), "{path:?}");
    }
    for files in [
        vec![("a", ""), ("a", "")],
        vec![("a/b", ""), ("a", "")],
        vec![("a", ""), ("a-else", ""), ("a/b/c", "")],
    ] {
        assert!(MemoryTree::new(files).is_err());
    }
}

struct MissingTree;

impl SourceTree for MissingTree {
    fn paths(&self) -> Result<Vec<String>, String> {
        Ok(vec![
            "tests/a.rs".into(),
            "checks/a.nix".into(),
            "e2e/a.ts".into(),
        ])
    }
    fn read(&self, _: &str) -> Result<String, String> {
        Err("authorized content unavailable".into())
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-SCANNING-TREES-003"))]
fn missing_virtual_content_is_an_error_for_each_scanner() {
    let errors = scan_tree(&MissingTree, &config()).expect_err("missing content");
    assert_eq!(errors.len(), 3);
    assert!(
        errors
            .iter()
            .all(|error| error.message.contains("authorized content unavailable"))
    );
    assert_eq!(
        errors
            .iter()
            .map(|error| error.code.as_str())
            .collect::<Vec<_>>(),
        ["NixScanError", "PlaywrightScanError", "RustScanError"]
    );
}

struct UnreadableTree;

impl SourceTree for UnreadableTree {
    fn paths(&self) -> Result<Vec<String>, String> {
        panic!("must not enumerate")
    }
    fn read(&self, _: &str) -> Result<String, String> {
        panic!("must not read")
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-SCANNING-TREES-004"))]
fn disabled_and_unmatched_scanners_are_explicit() {
    let mut config = config();
    config.rust.enabled = false;
    config.playwright.enabled = false;
    config.nix.enabled = false;
    let disabled = scan_tree(&UnreadableTree, &config).expect("disabled");
    assert!(!disabled.is_complete());
    assert!(
        disabled
            .scanners
            .iter()
            .all(|scanner| scanner.state == DiscoveryState::Disabled)
    );
    config.rust.enabled = true;
    let empty = scan_tree(
        &MemoryTree::new(Vec::<(String, String)>::new()).expect("empty"),
        &config,
    )
    .expect("unmatched");
    assert_eq!(empty.scanners[0].state, DiscoveryState::NoMatchingFiles);
    assert!(!empty.is_complete());
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-SCANNING-TREES-005"))]
fn invalid_configuration_precedes_tree_access() {
    let mut invalid = config();
    invalid.rust.paths = vec!["../escape.rs".into()];
    assert_eq!(
        scan_tree(&UnreadableTree, &invalid).expect_err("parent path")[0].code,
        "ConfigurationError"
    );
    invalid = config();
    invalid.playwright.tag_prefix.clear();
    assert_eq!(
        scan_tree(&UnreadableTree, &invalid).expect_err("empty prefix")[0].code,
        "ConfigurationError"
    );
    invalid = config();
    invalid.nix.paths.clear();
    assert_eq!(
        scan_tree(&UnreadableTree, &invalid).expect_err("empty paths")[0].code,
        "ConfigurationError"
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-SCANNING-TREES-006"))]
fn relevant_source_and_configuration_changes_change_discovery() {
    let _guard = crate::tests::scan_lock();
    let memory = MemoryTree::new(FILES.iter().copied()).expect("tree");
    let original = scan_tree(&memory, &config()).expect("scan");
    let changed = MemoryTree::new(
        FILES
            .iter()
            .map(|(path, source)| (*path, source.replace("BIND-001", "BIND-002"))),
    )
    .expect("changed tree");
    assert_ne!(
        original,
        scan_tree(&changed, &config()).expect("changed bindings")
    );
    let mut changed_config = config();
    changed_config.rust.enabled = false;
    assert_ne!(
        original,
        scan_tree(&memory, &changed_config).expect("disabled scanner")
    );
    changed_config = config();
    changed_config.playwright.tag_prefix = "@other:".into();
    assert_ne!(
        original,
        scan_tree(&memory, &changed_config).expect("changed prefix")
    );
}

#[cfg(unix)]
#[test]
#[cfg_attr(any(), bloomery("PARSER-SCANNING-TREES-007"))]
fn filesystem_discovery_excludes_symlinks_and_cycles() {
    let disk = Fixture::new(&[("tests/real.rs", "#[test] fn real() {}")]);
    std::os::unix::fs::symlink("real.rs", disk.0.join("tests/link.rs")).expect("file link");
    std::os::unix::fs::symlink(".", disk.0.join("tests/cycle")).expect("directory cycle");
    let filesystem = FilesystemTree::new(&disk.0);
    assert_eq!(filesystem.paths().expect("index"), ["tests/real.rs"]);
    assert_eq!(
        scan_tree(&filesystem, &config())
            .expect("scan")
            .output
            .tests
            .len(),
        1
    );
}
