//! Standalone consumer of the public source discovery API.
use bloomery_model::{Config, Context, config::ScannersConfig};
use bloomery_scanning::{FilesystemTree, MemoryTree, SourceTree, scan_all, scan_tree};
use std::fs;
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).expect("fixture root"));
    let files = [
        (
            "tests/a.rs",
            "#[cfg_attr(any(), bloomery(\"DEMO-TESTS-BIND-001\"))]\n#[test]\nfn tied() { panic!(\"never run\"); }\n#[test]\nfn untied() {}\n",
        ),
        (
            "e2e/a.ts",
            "test('tied', { tag: ['@bloomery:DEMO-TESTS-BIND-001'] }, async () => { throw new Error('never run'); });\ntest('untied', async () => {});\n",
        ),
        (
            "checks/a.test.nix",
            "{ passthru.bloomery = [ \"DEMO-TESTS-BIND-001\" ]; }\n",
        ),
        (
            "checks/untied.test.nix",
            "builtins.abort \"never evaluate\"\n",
        ),
    ];
    for (path, source) in &files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().expect("parent")).expect("directory");
        fs::write(path, source).expect("source");
    }
    let mut config = ScannersConfig::default();
    config.rust.paths = vec!["tests/**/*.rs".into()];
    config.playwright.paths = vec!["e2e/**/*.ts".into()];
    config.nix.paths = vec!["checks/**/*.nix".into()];
    config.nix.test_paths = vec!["checks/**/*.test.nix".into()];
    let memory = MemoryTree::new(files).expect("virtual tree");
    let virtual_analysis = scan_tree(&memory, &config).expect("virtual analysis");
    let disk = FilesystemTree::new(&root);
    assert_eq!(
        virtual_analysis,
        scan_tree(&disk, &config).expect("filesystem analysis")
    );
    assert!(virtual_analysis.is_complete());
    let context = Context {
        specs_enabled: true,
        root: root.clone(),
        config: Config {
            scanners: config.clone(),
            ..Config::default()
        },
        areas: Vec::new(),
    };
    let mut output = scan_all(&context).expect("filesystem compatibility API");
    for item in &mut output.evidence {
        item.location.path = item
            .location
            .path
            .strip_prefix(&root)
            .expect("relative")
            .into();
    }
    for item in &mut output.tests {
        item.location.path = item
            .location
            .path
            .strip_prefix(&root)
            .expect("relative")
            .into();
    }
    assert_eq!(output, virtual_analysis.output);
    check_specification_parity(&root, &config);
    // Export only the fields supported by the existing CLI projections.
    for item in &output.evidence {
        println!(
            "reference\t{}\t{}\t{}",
            item.scanner,
            item.location.path.display(),
            item.location.line.expect("line")
        );
    }
    for site in &output.tests {
        if site.references.is_empty() {
            println!(
                "candidate\t{}\t{}\t{}\t{}",
                site.scanner,
                site.location.path.display(),
                site.location.line.expect("line"),
                site.name.as_deref().unwrap_or("")
            );
        }
    }
    // A new input snapshot must be read by a new analysis, never a global cache.
    fs::write(root.join("tests/a.rs"), "#[test] fn changed() {}\n").expect("changed source");
    assert_ne!(
        virtual_analysis,
        scan_tree(&disk, &config).expect("fresh analysis")
    );
    assert_eq!(
        virtual_analysis,
        scan_tree(&memory, &config).expect("immutable snapshot")
    );
    fs::write(
        root.join("tests/a.rs"),
        memory.read("tests/a.rs").expect("original"),
    )
    .expect("restore fixture");
    let malformed = MemoryTree::new(files.iter().map(|(path, source)| {
        (
            *path,
            if *path == "tests/a.rs" {
                "fn broken( {"
            } else {
                source
            },
        )
    }))
    .expect("malformed snapshot");
    fs::write(root.join("tests/a.rs"), "fn broken( {").expect("malformed source");
    let virtual_errors = scan_tree(&malformed, &config).expect_err("virtual parse error");
    let disk_errors = scan_tree(&disk, &config).expect_err("filesystem parse error");
    assert_eq!(virtual_errors.len(), disk_errors.len());
    for (a, b) in virtual_errors.iter().zip(&disk_errors) {
        assert_eq!(
            (&a.code, &a.message, &a.location, &a.notes),
            (&b.code, &b.message, &b.location, &b.notes)
        );
        let location = a.location.as_ref().expect("location");
        println!(
            "diagnostic\t{}\t{}\t{}\t{}",
            a.code,
            location.path.display(),
            location.line.expect("line"),
            a.message
        );
    }
    fs::write(
        root.join("tests/a.rs"),
        memory.read("tests/a.rs").expect("original"),
    )
    .expect("restore fixture");
}

fn repository_snapshot(disk: &FilesystemTree) -> MemoryTree {
    MemoryTree::with_directories(
        disk.paths()
            .expect("repository paths")
            .into_iter()
            .filter(|path| path != ".bloomery/config.toml")
            .map(|path| {
                let text = disk.read(&path).expect("repository content");
                (path, text)
            }),
        disk.directories().expect("repository directories"),
    )
    .expect("repository snapshot")
}

fn relative_diagnostics(
    root: &std::path::Path,
    mut diagnostics: Vec<bloomery_model::Diagnostic>,
) -> Vec<bloomery_model::Diagnostic> {
    for diagnostic in &mut diagnostics {
        if let Some(location) = &mut diagnostic.location {
            location.path = location
                .path
                .strip_prefix(root)
                .expect("relative diagnostic")
                .into();
        }
    }
    diagnostics
}

fn check_specification_parity(root: &std::path::Path, scanners: &ScannersConfig) {
    let disk = FilesystemTree::new(root);
    let memory = repository_snapshot(&disk);
    let mut config = Config {
        scanners: scanners.clone(),
        ..Config::default()
    };
    config.specs.dir = "custom".into();
    let virtual_context =
        bloomery_workspace::load_tree(&memory, config.clone()).expect("virtual specs");
    let filesystem_context = bloomery_workspace::load(root).expect("filesystem specs");
    let relative_context =
        bloomery_workspace::load_tree(&disk, config.clone()).expect("filesystem tree specs");
    assert_eq!(virtual_context.areas, relative_context.areas);
    assert!(virtual_context.spec_management_enabled());
    let virtual_scan = scan_tree(&memory, scanners).expect("virtual discovery");
    let filesystem_scan = scan_tree(&disk, scanners).expect("filesystem discovery");
    assert_eq!(virtual_scan, filesystem_scan);
    bloomery_check::validate_evidence(&virtual_context, &virtual_scan.output.evidence)
        .expect("virtual static relations");
    bloomery_check::run(&filesystem_context).expect("filesystem static relations");
    assert_eq!(
        bloomery_check::validate_evidence(&virtual_context, &[]).expect_err("missing bindings"),
        relative_diagnostics(
            root,
            bloomery_check::validate_evidence(&filesystem_context, &[])
                .expect_err("missing bindings")
        ),
    );
    let mut unknown = virtual_scan.output.evidence.clone();
    let mut reference = unknown[0].clone();
    reference.id = "DEMO-TESTS-BIND-999".into();
    unknown.push(reference);
    assert_eq!(
        bloomery_check::validate_evidence(&virtual_context, &unknown).expect_err("orphan")[0].code,
        "OrphanTestReference"
    );
    // A lifecycle edit is an ordinary repository edit, seen by both loaders.
    let group = ".bloomery/custom/DEMO/TESTS/requirements/BIND.toml";
    let original = disk.read(group).expect("original spec");
    let changed = original.replace("manual = false", "manual = true");
    std::fs::write(root.join(group), &changed).expect("repository edit");
    let edited_memory = repository_snapshot(&disk);
    let edited_virtual =
        bloomery_workspace::load_tree(&edited_memory, config).expect("edited virtual specs");
    let edited_filesystem = bloomery_workspace::load(root).expect("edited filesystem specs");
    assert_ne!(virtual_context.areas, edited_virtual.areas);
    let virtual_errors =
        bloomery_check::validate_evidence(&edited_virtual, &virtual_scan.output.evidence)
            .expect_err("manual automation");
    assert!(
        virtual_errors
            .iter()
            .all(|error| error.code == "IllegalManualAutomation")
    );
    assert_eq!(
        virtual_errors,
        relative_diagnostics(
            root,
            bloomery_check::run(&edited_filesystem).expect_err("manual automation")
        )
    );
    std::fs::write(root.join(group), original).expect("restore spec");
}
