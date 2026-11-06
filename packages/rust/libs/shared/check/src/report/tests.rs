use super::*;
use bloomery_workspace::source::MemoryTree;
use std::cell::RefCell;
use std::collections::BTreeMap;

const ID: &str = "DEMO-TESTS-BIND-001";
const ROOT: &str = ".bloomery/specs/DEMO/TESTS";

fn config() -> Config {
    let mut config = Config::default();
    config.scanners.rust.paths = vec!["tests/**/*.rs".into()];
    config.scanners.playwright.paths = vec!["e2e/**/*.ts".into()];
    config.scanners.nix.paths = vec!["checks/**/*.nix".into()];
    config.scanners.nix.test_paths = vec!["checks/**/*.test.nix".into()];
    config
}

fn record(id: &str, manual: bool) -> String {
    format!(
        "[[requirements]]\nid = \"{id}\"\ntitle = \"Binding\"\nmanual = {manual}\ndesign = \"design/main.md\"\nears = {{ type = \"ubiquitous\", system = \"example\", action = \"work\" }}\n"
    )
}

fn files() -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    for (path, id) in [(".bloomery/specs/DEMO", "DEMO"), (ROOT, "TESTS")] {
        files.insert(format!("{path}/README.md"), format!("---\nid: {id}\nname: Example\ntagline: Example\ndescription: Example\n---\n# Example\n"));
    }
    files.insert(format!("{ROOT}/design/main.md"), "# Main\n".into());
    files.insert(
        format!("{ROOT}/requirements/BIND.toml"),
        format!("group = \"BIND\"\n{}", record(ID, false)),
    );
    files.insert("tests/a.rs".into(), format!("#[cfg_attr(any(), bloomery(\"{ID}\"))]\n#[test]\nfn tied() {{ panic!(\"never run\"); }}\n#[test]\nfn untied() {{}}\n"));
    files.insert("e2e/a.ts".into(), format!("test('tied', {{ tag: ['@bloomery:{ID}'] }}, async () => {{ throw Error('never run'); }});\n"));
    files.insert(
        "checks/a.test.nix".into(),
        format!("{{ passthru.bloomery = [ \"{ID}\" ]; }}\n"),
    );
    files
}

fn report(files: BTreeMap<String, String>, config: Config) -> StaticReport {
    analyze_tree(&MemoryTree::new(files).expect("tree"), config)
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-003"))]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-007"))]
fn complete_report_preserves_static_facts_scope_and_source_positions() {
    let report = report(files(), config());
    assert_eq!(report.version, REPORT_VERSION);
    assert_eq!(report.completeness.relations, StageState::Complete);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    assert_eq!(report.requirements[0].coverage, Coverage::Bound);
    assert_eq!(report.tests.len(), 4);
    assert_eq!(report.bindings.len(), 3);
    assert!(
        report
            .bindings
            .iter()
            .all(|binding| binding.resolution == BindingResolution::Declared)
    );
    assert!(
        report
            .bindings
            .iter()
            .all(|binding| binding.location.line.is_some())
    );
    assert!(
        report
            .bindings
            .iter()
            .all(|binding| binding.location.column.is_none())
    );
    assert_eq!(report.scope.kind, "syntactic");
    assert_eq!(report.scope.target_resolution, "unsupported");
    assert_eq!(report.scope.feature_resolution, "unsupported");
    assert_eq!(report.scope.conditional_compilation, "unsupported");
    let json = serde_json::to_value(&report).expect("JSON");
    for key in ["passed", "approval", "execution"] {
        assert!(json.get(key).is_none());
    }
    assert_eq!(report.inputs.parser_contract, PARSER_CONTRACT);
    assert_eq!(report.inputs.specification_schema, SPECIFICATION_SCHEMA);
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-002"))]
fn identities_are_canonical_and_bind_relevant_inputs() {
    let original = report(files(), config());
    let reversed =
        MemoryTree::with_directories(files().into_iter().rev(), ["tests", "checks", "e2e"])
            .expect("reordered tree");
    assert_eq!(original, analyze_tree(&reversed, config()));
    let mut changed = files();
    changed.insert("unselected.bin".into(), "different tree".into());
    let changed = report(changed, config());
    assert_ne!(original.identity, changed.identity);
    assert_ne!(original.inputs.tree, changed.inputs.tree);
    assert_eq!(
        original.inputs.specifications,
        changed.inputs.specifications
    );
    assert_eq!(original.inputs.configuration, changed.inputs.configuration);
    let mut changed_config = config();
    changed_config.scanners.playwright.tag_prefix = "@other:".into();
    let changed = report(files(), changed_config);
    assert_eq!(original.inputs.tree, changed.inputs.tree);
    assert_ne!(original.inputs.configuration, changed.inputs.configuration);
    assert_ne!(original.identity, changed.identity);
    let mut changed_files = files();
    changed_files.insert(
        format!("{ROOT}/design/main.md"),
        "# Main\nRevised design\n".into(),
    );
    let changed = report(changed_files, config());
    assert_ne!(
        original.inputs.specifications,
        changed.inputs.specifications
    );
    let empty = MemoryTree::with_directories(files(), ["empty"]).expect("empty directory");
    assert_ne!(
        original.inputs.tree,
        analyze_tree(&empty, config()).inputs.tree
    );
    let binary = |bytes| {
        MemoryTree::from_bytes(
            files()
                .into_iter()
                .map(|(path, text)| (path, text.into_bytes()))
                .chain([("asset.bin".into(), bytes)]),
            Vec::<String>::new(),
        )
        .expect("binary tree")
    };
    let first = analyze_tree(&binary(vec![0, 255]), config());
    let second = analyze_tree(&binary(vec![0, 254]), config());
    assert!(first.diagnostics.is_empty());
    assert_ne!(first.inputs.tree, second.inputs.tree);
    assert_eq!(first.tests, second.tests);
    let mut payload = serde_json::to_value(&original).expect("payload");
    payload.as_object_mut().expect("object").remove("identity");
    assert_eq!(original.identity, identity(&payload));
    payload["inputs"]["parser_contract"] = "next-parser".into();
    assert_ne!(original.identity, identity(&payload));
    payload["inputs"]["parser_contract"] = PARSER_CONTRACT.into();
    payload["inputs"]["specification_schema"] = "next-schema".into();
    assert_ne!(original.identity, identity(&payload));
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-003"))]
fn relational_findings_preserve_missing_manual_and_unknown_references() {
    let mut files = files();
    files
        .get_mut(&format!("{ROOT}/requirements/BIND.toml"))
        .expect("group")
        .push_str(&format!(
            "{}{}",
            record("DEMO-TESTS-BIND-002", false),
            record("DEMO-TESTS-BIND-003", true)
        ));
    files.get_mut("tests/a.rs").expect("source").push_str("#[cfg_attr(any(), bloomery(\"DEMO-TESTS-BIND-003\"))]\n#[cfg_attr(any(), bloomery(\"DEMO-TESTS-BIND-999\"))]\n#[test]\nfn invalid() {}\n");
    let report = report(files, config());
    assert_eq!(report.completeness.relations, StageState::Complete);
    assert_eq!(report.requirements[1].coverage, Coverage::Missing);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|finding| finding.code == "MissingAutomatedTest")
    );
    let manual = report
        .diagnostics
        .iter()
        .find(|finding| finding.code == "IllegalManualAutomation")
        .expect("manual finding");
    assert!(!manual.notes.is_empty());
    assert!(
        report
            .diagnostics
            .iter()
            .any(|finding| finding.code == "OrphanTestReference")
    );
    assert!(
        report
            .bindings
            .iter()
            .any(|binding| binding.resolution == BindingResolution::Unresolved)
    );
    assert!(
        report
            .bindings
            .iter()
            .any(|binding| binding.resolution == BindingResolution::Manual)
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-004"))]
fn incomplete_discovery_leaves_absent_bindings_unresolved() {
    let mut files = files();
    files
        .get_mut(&format!("{ROOT}/requirements/BIND.toml"))
        .expect("group")
        .push_str(&record("DEMO-TESTS-BIND-002", false));
    let mut disabled = config();
    disabled.scanners.rust.enabled = false;
    let report = report(files.clone(), disabled);
    assert_eq!(report.scanners[0].state, ScannerState::Disabled);
    assert_eq!(report.completeness.discovery, StageState::Partial);
    assert_eq!(report.requirements[0].coverage, Coverage::Bound);
    assert_eq!(report.requirements[1].coverage, Coverage::Unresolved);
    assert!(report.diagnostics.is_empty());
    let mut unmatched = config();
    unmatched.scanners.rust.paths = vec!["missing/**/*.rs".into()];
    let report = self::report(files, unmatched);
    assert_eq!(report.scanners[0].state, ScannerState::NoMatchingFiles);
    assert_eq!(report.completeness.relations, StageState::Partial);
    assert_eq!(report.requirements[1].coverage, Coverage::Unresolved);
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-006"))]
fn malformed_specs_and_sources_retain_failures_without_dependent_claims() {
    for document in [
        "invalid TOML [".into(),
        format!(
            "group = \"BIND\"\n{}{}",
            record(ID, false),
            record(ID, false)
        ),
    ] {
        let mut files = files();
        files.insert(format!("{ROOT}/requirements/BIND.toml"), document);
        let report = report(files, config());
        assert_eq!(report.completeness.specifications, StageState::Failed);
        assert_eq!(report.inputs.specifications, None);
        assert_eq!(report.completeness.relations, StageState::NotAttempted);
        assert!(!report.diagnostics.is_empty());
        assert!(
            report
                .bindings
                .iter()
                .all(|binding| binding.resolution == BindingResolution::SpecificationsUnavailable)
        );
    }
    let mut files = files();
    files.insert("tests/a.rs".into(), "fn broken( {".into());
    let failed = report(files.clone(), config());
    assert_eq!(failed.completeness.discovery, StageState::Failed);
    assert_eq!(failed.completeness.relations, StageState::NotAttempted);
    assert!(failed.tests.is_empty());
    assert_eq!(failed.requirements[0].coverage, Coverage::Unresolved);
    assert!(!failed.diagnostics.is_empty());
    let mut disabled = config();
    disabled.specs.enable = Some(false);
    files.insert(
        format!("{ROOT}/requirements/BIND.toml"),
        "invalid TOML [".into(),
    );
    files.insert("tests/a.rs".into(), "#[test] fn untied() {}".into());
    let disabled = report(files, disabled);
    assert_eq!(disabled.completeness.specifications, StageState::Disabled);
    assert_eq!(disabled.completeness.relations, StageState::NotAttempted);
    assert!(disabled.requirements.is_empty());
    assert!(disabled.diagnostics.is_empty());
}

struct CountingTree {
    memory: MemoryTree,
    reads: RefCell<BTreeMap<String, usize>>,
}

impl SourceTree for CountingTree {
    fn paths(&self) -> Result<Vec<String>, String> {
        self.memory.paths()
    }
    fn read(&self, path: &str) -> Result<String, String> {
        *self.reads.borrow_mut().entry(path.into()).or_default() += 1;
        self.memory.read(path)
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-002"))]
fn parsers_and_identity_share_one_content_snapshot() {
    let tree = CountingTree {
        memory: MemoryTree::new(files()).expect("tree"),
        reads: RefCell::new(BTreeMap::new()),
    };
    let original = analyze_tree(&tree, config());
    assert!(original.diagnostics.is_empty());
    assert_eq!(tree.reads.borrow().len(), files().len());
    assert!(tree.reads.borrow().values().all(|count| *count == 1));
    let mut changed = files();
    changed.insert("tests/a.rs".into(), "#[test] fn changed() {}".into());
    let changed = report(changed, config());
    assert_ne!(original.inputs.tree, changed.inputs.tree);
    assert_ne!(original.tests, changed.tests);
    assert_ne!(original.identity, changed.identity);
}

struct UnreliableTree {
    paths: Vec<String>,
    reads: RefCell<Vec<String>>,
}

impl SourceTree for UnreliableTree {
    fn paths(&self) -> Result<Vec<String>, String> {
        Ok(self.paths.clone())
    }
    fn read(&self, path: &str) -> Result<String, String> {
        self.reads.borrow_mut().push(path.into());
        Err("content unavailable".into())
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-005"))]
fn missing_content_and_path_collisions_fail_before_downstream_analysis() {
    for paths in [
        vec!["missing.rs"],
        vec!["same", "same"],
        vec!["parent", "parent/child"],
        vec!["../escape"],
    ] {
        let tree = UnreliableTree {
            paths: paths.into_iter().map(String::from).collect(),
            reads: RefCell::new(Vec::new()),
        };
        let report = analyze_tree(&tree, config());
        assert_eq!(report.completeness.tree, StageState::Failed);
        assert_eq!(report.inputs.tree, None);
        assert_eq!(report.completeness.discovery, StageState::NotAttempted);
        assert!(report.bindings.is_empty());
        assert_eq!(report.diagnostics[0].code, "SourceTreeError");
        assert!(tree.reads.borrow().len() <= 1);
    }
    let tree = UnreliableTree {
        paths: vec!["missing.rs".into()],
        reads: RefCell::new(Vec::new()),
    };
    let mut invalid = config();
    invalid.version = 999;
    let report = analyze_tree(&tree, invalid);
    assert_eq!(report.completeness.configuration, StageState::Failed);
    assert!(tree.reads.borrow().is_empty());
}
