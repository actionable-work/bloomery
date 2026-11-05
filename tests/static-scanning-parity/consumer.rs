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
