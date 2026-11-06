use crate::{
    load, load_tree,
    source::{FilesystemTree, MemoryTree, SourceTree, TreeIndex},
};
use bloomery_model::{Config, Context, Diagnostic};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        Self(crate::tests::fixture())
    }

    fn snapshot(&self) -> MemoryTree {
        let disk = FilesystemTree::new(&self.0);
        MemoryTree::with_directories(
            disk.paths()
                .expect("paths")
                .into_iter()
                .filter(|path| path != ".bloomery/config.toml")
                .map(|path| {
                    let source = disk.read(&path).expect("content");
                    (path, source)
                }),
            disk.directories().expect("directories"),
        )
        .expect("snapshot")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("cleanup");
    }
}

const GROUP: &str = ".bloomery/specs/PARSER/WORKSPACE/requirements/STRUCTURE.toml";
const DESIGN: &str = ".bloomery/specs/PARSER/WORKSPACE/design/overview.md";

fn relative(mut context: Context) -> Context {
    let root = context.root.clone();
    for area in &mut context.areas {
        area.path = area.path.strip_prefix(&root).expect("relative").into();
        for feature in &mut area.features {
            feature.path = feature.path.strip_prefix(&root).expect("relative").into();
            for group in &mut feature.groups {
                group.path = group.path.strip_prefix(&root).expect("relative").into();
                for requirement in &mut group.requirements {
                    requirement.path = requirement
                        .path
                        .strip_prefix(&root)
                        .expect("relative")
                        .into();
                    requirement.design.path = requirement
                        .design
                        .path
                        .strip_prefix(&root)
                        .expect("relative")
                        .into();
                }
            }
        }
    }
    context.root = PathBuf::new();
    context
}

fn relative_errors(root: &Path, mut errors: Vec<Diagnostic>) -> Vec<Diagnostic> {
    for error in &mut errors {
        if let Some(location) = &mut error.location {
            location.path = location.path.strip_prefix(root).expect("relative").into();
        }
    }
    errors
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-TREES-001"))]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-TREES-002"))]
fn adapters_discover_the_same_repository_definitions_without_virtual_configuration_files() {
    let fixture = Fixture::new();
    let memory = fixture.snapshot();
    let virtual_context = load_tree(&memory, Config::default()).expect("virtual specs");
    let disk =
        load_tree(&FilesystemTree::new(&fixture.0), Config::default()).expect("filesystem tree");
    let compatibility = relative(load(&fixture.0).expect("filesystem loader"));
    assert_eq!(virtual_context.areas, disk.areas);
    assert_eq!(virtual_context.areas, compatibility.areas);
    assert!(virtual_context.root.as_os_str().is_empty());
    assert!(virtual_context.spec_management_enabled());
    let reversed = MemoryTree::with_directories(
        memory
            .paths()
            .expect("paths")
            .into_iter()
            .rev()
            .map(|path| {
                let text = memory.read(&path).expect("text");
                (path, text)
            }),
        memory.directories().expect("directories").into_iter().rev(),
    )
    .expect("reverse tree");
    assert_eq!(
        virtual_context.areas,
        load_tree(&reversed, Config::default())
            .expect("reverse load")
            .areas
    );
    let changed = MemoryTree::with_directories(
        memory.paths().expect("paths").into_iter().map(|path| {
            let text = memory
                .read(&path)
                .expect("text")
                .replace("retain structure", "validate repository changes");
            (path, text)
        }),
        memory.directories().expect("directories"),
    )
    .expect("changed repository");
    assert_ne!(
        virtual_context.areas,
        load_tree(&changed, Config::default())
            .expect("changed load")
            .areas
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
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-TREES-003"))]
fn disabled_specs_and_invalid_config_do_not_access_tree_content() {
    let mut config = Config::default();
    config.specs.enable = Some(false);
    let context = load_tree(&UnreadableTree, config).expect("disabled");
    assert!(context.areas.is_empty());
    assert!(!context.spec_management_enabled());
    let mut invalid = Config::default();
    invalid.version = 999;
    assert_eq!(
        load_tree(&UnreadableTree, invalid).expect_err("invalid version")[0].code,
        "ConfigurationError"
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-TREES-004"))]
fn only_the_configured_static_root_is_active() {
    let fixture = Fixture::new();
    let original = fixture.snapshot();
    let files = original
        .paths()
        .expect("paths")
        .into_iter()
        .map(|path| {
            let text = original.read(&path).expect("content");
            (path.replace(".bloomery/specs/", ".bloomery/custom/"), text)
        })
        .chain([(GROUP.to_owned(), "invalid legacy TOML [".to_owned())]);
    let dirs = original
        .directories()
        .expect("directories")
        .into_iter()
        .map(|path| path.replace(".bloomery/specs", ".bloomery/custom"));
    let memory = MemoryTree::with_directories(files, dirs).expect("custom tree");
    let mut config = Config::default();
    config.specs.dir = "custom".into();
    let context = load_tree(&memory, config).expect("custom specs");
    assert_eq!(context.requirements().count(), 1);
    assert!(
        context
            .requirements()
            .next()
            .expect("requirement")
            .2
            .path
            .starts_with(".bloomery/custom")
    );
    let empty = MemoryTree::new(Vec::<(String, String)>::new()).expect("empty tree");
    assert!(
        !load_tree(&empty, Config::default())
            .expect("auto disabled")
            .spec_management_enabled()
    );
    let mut required = Config::default();
    required.specs.enable = Some(true);
    assert_eq!(
        load_tree(&empty, required).expect_err("required tree")[0].code,
        "MissingSpecsDirectory"
    );
    let empty_root =
        MemoryTree::with_directories(Vec::<(String, String)>::new(), [".bloomery/specs"])
            .expect("empty root");
    assert!(
        load_tree(&empty_root, Config::default())
            .expect("enabled empty root")
            .spec_management_enabled()
    );
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-TREES-005"))]
fn invalid_specs_and_design_links_have_identical_diagnostics() {
    let fixture = Fixture::new();
    let original = fs::read_to_string(fixture.0.join(GROUP)).expect("group");
    let record = original.split_once("[[requirements]]").expect("record").1;
    for invalid in [
        "invalid TOML [".to_owned(),
        format!("{original}\n[[requirements]]{record}"),
        original.replace(
            "manual = true",
            "manual = true\ndesign = \"design/missing.md\"",
        ),
        original.replace(
            "manual = true",
            "manual = true\ndesign = \"design/overview.md#missing\"",
        ),
        original.replace("manual = true", "manual = true\ndesign = \"../escape.md\""),
    ] {
        fs::write(fixture.0.join(GROUP), invalid).expect("invalid fixture");
        let memory = fixture.snapshot();
        let virtual_errors =
            load_tree(&memory, Config::default()).expect_err("invalid virtual specs");
        assert_eq!(
            virtual_errors,
            load_tree(&FilesystemTree::new(&fixture.0), Config::default())
                .expect_err("invalid filesystem specs")
        );
        assert_eq!(
            virtual_errors,
            relative_errors(
                &fixture.0,
                load(&fixture.0).expect_err("invalid filesystem adapter")
            )
        );
    }
    fs::write(
        fixture.0.join(GROUP),
        original.replace(
            "manual = true",
            "manual = true\ndesign = \"design/overview.md#overview\"",
        ),
    )
    .expect("valid anchor");
    assert!(load_tree(&fixture.snapshot(), Config::default()).is_ok());
}

struct CountingTree {
    memory: MemoryTree,
    unavailable: Option<String>,
    reads: RefCell<BTreeMap<String, usize>>,
}
impl SourceTree for CountingTree {
    fn paths(&self) -> Result<Vec<String>, String> {
        self.memory.paths()
    }
    fn directories(&self) -> Result<Vec<String>, String> {
        self.memory.directories()
    }
    fn read(&self, path: &str) -> Result<String, String> {
        *self.reads.borrow_mut().entry(path.to_owned()).or_default() += 1;
        if self.unavailable.as_deref() == Some(path) {
            return Err("authorized content unavailable".into());
        }
        self.memory.read(path)
    }
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-TREES-006"))]
fn unavailable_selected_documents_fail_and_document_reads_are_cached() {
    let fixture = Fixture::new();
    let original = fs::read_to_string(fixture.0.join(GROUP)).expect("group");
    // Two requirements reference the same design file without an anchor.
    let record = original
        .split_once("[[requirements]]")
        .expect("record")
        .1
        .replace("STRUCTURE-001", "STRUCTURE-002");
    fs::write(
        fixture.0.join(GROUP),
        format!("{original}\n[[requirements]]{record}").replace(
            "manual = true",
            "manual = true\ndesign = \"design/overview.md\"",
        ),
    )
    .expect("shared design");
    for unavailable in [".bloomery/specs/PARSER/README.md", GROUP, DESIGN] {
        let tree = CountingTree {
            memory: fixture.snapshot(),
            unavailable: Some(unavailable.into()),
            reads: RefCell::new(BTreeMap::new()),
        };
        let errors = load_tree(&tree, Config::default()).expect_err("missing content");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("authorized content unavailable"))
        );
        assert_eq!(tree.reads.borrow().get(unavailable), Some(&1));
    }
    let tree = CountingTree {
        memory: fixture.snapshot(),
        unavailable: None,
        reads: RefCell::new(BTreeMap::new()),
    };
    assert!(load_tree(&tree, Config::default()).is_ok());
    assert!(tree.reads.borrow().values().all(|count| *count == 1));
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-TREES-007"))]
fn explicit_directories_preserve_empty_design_folders_and_reject_collisions() {
    let fixture = Fixture::new();
    fs::remove_file(fixture.0.join(DESIGN)).expect("empty design directory");
    let memory = fixture.snapshot();
    assert!(
        TreeIndex::new(&memory)
            .expect("index")
            .is_dir(Path::new(".bloomery/specs/PARSER/WORKSPACE/design"))
    );
    assert_eq!(
        load_tree(&memory, Config::default())
            .expect("virtual empty design")
            .areas,
        relative(load(&fixture.0).expect("filesystem empty design")).areas
    );
    assert!(MemoryTree::with_directories([("a", "")], ["a"]).is_err());
    assert!(MemoryTree::with_directories([("a", "")], ["a/b"]).is_err());
    assert!(MemoryTree::with_directories(Vec::<(String, String)>::new(), ["a", "a"]).is_err());
    assert!(MemoryTree::with_directories(Vec::<(String, String)>::new(), ["../escape"]).is_err());
}

#[test]
#[cfg_attr(any(), bloomery("PARSER-WORKSPACE-REPORT-002"))]
fn byte_trees_preserve_binary_content_and_canonical_directories() {
    let bytes = vec![0, 255, 128];
    let tree =
        MemoryTree::from_bytes([("images/icon", bytes.clone())], ["empty"]).expect("byte tree");
    assert_eq!(tree.read_bytes("images/icon").expect("bytes"), bytes);
    assert!(tree.read("images/icon").is_err());
    let index = TreeIndex::new(&tree).expect("index");
    assert_eq!(index.files().collect::<Vec<_>>(), ["images/icon"]);
    assert_eq!(index.directories().collect::<Vec<_>>(), ["empty", "images"]);
    let fixture = Fixture::new();
    fs::write(fixture.0.join("binary"), &bytes).expect("binary file");
    let disk = FilesystemTree::new(&fixture.0);
    assert_eq!(disk.read_bytes("binary").expect("filesystem bytes"), bytes);
    assert!(disk.read("binary").is_err());
}
