use super::check_workspace_unused_dependencies;
use bloomery_model::Diagnostic;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static FIXTURE_ID: AtomicUsize = AtomicUsize::new(0);

const APP_MANIFEST: &str = "[package]
name = \"app\"
version = \"0.1.0\"
edition = \"2024\"

[dependencies]
helper = { path = \"../helper\" }
serde = \"1\"
json = { package = \"serde_json\", version = \"1\" }
log = \"0.4\"
r-hyphen = { package = \"regex\", version = \"1\" }

[dev-dependencies]
tempfile = \"3\"
anyhow = \"1\"

[build-dependencies]
cc = \"1\"
";

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(files: &[(&str, &str)]) -> Self {
        let id = FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("bloomery-unused-deps-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let defaults = [
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"app\", \"helper\"]\nresolver = \"2\"\n",
            ),
            (
                "helper/Cargo.toml",
                "[package]\nname = \"helper\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\nname = \"helper_core\"\n",
            ),
            ("helper/src/lib.rs", "pub fn help() {}\n"),
            ("app/Cargo.toml", APP_MANIFEST),
            ("app/build.rs", "fn main() {}\n"),
        ];
        for (path, contents) in defaults.iter().chain(files) {
            let path = root.join(path);
            fs::create_dir_all(path.parent().expect("parent")).expect("directory");
            fs::write(path, contents).expect("fixture file");
        }
        Self { root }
    }

    fn check(&self, exceptions: &[&str]) -> Vec<Diagnostic> {
        let exceptions = exceptions
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>();
        check_workspace_unused_dependencies(&self.root, &exceptions)
            .err()
            .unwrap_or_default()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn used_sources() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "app/src/lib.rs",
            "mod nested;\n#[cfg(test)]\nmod tests { use tempfile as _; }\n",
        ),
        ("app/src/nested.rs", "pub fn f() { helper_core::help(); }\n"),
        (
            "app/src/main.rs",
            "fn main() { println!(\"{:?}\", json::Value::Null); let _ = r_hyphen::Regex::new(\"\"); }\n",
        ),
        (
            "app/tests/integration.rs",
            "#[derive(serde::Serialize)]\nstruct S;\nuse anyhow as _;\n",
        ),
    ]
}

fn unused_messages(diagnostics: &[Diagnostic]) -> Vec<String> {
    let mut messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "UnusedDependency")
        .map(|diagnostic| diagnostic.message.clone())
        .collect::<Vec<_>>();
    messages.sort();
    messages
}

fn location(diagnostic: &Diagnostic) -> (&Path, Option<usize>) {
    let location = diagnostic.location.as_ref().expect("location");
    (location.path.as_path(), location.line)
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-002"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-003"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-005"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-006"))]
#[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-007"))]
fn reports_each_declaration_unreferenced_by_its_visible_targets() {
    let fixture = Fixture::new(&used_sources());
    let diagnostics = fixture.check(&[]);
    assert_eq!(
        unused_messages(&diagnostics),
        [
            "app declares unused build-dependencies entry 'cc'",
            "app declares unused dependencies entry 'log'",
            "app declares unused dependencies entry 'serde'",
        ],
        "{diagnostics:?}"
    );
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-004"))]
fn development_dependencies_are_unused_without_any_target_reference() {
    let mut sources = used_sources();
    sources.retain(|(path, _)| *path != "app/tests/integration.rs");
    sources.push((
        "app/tests/integration.rs",
        "#[derive(serde::Serialize)]\nstruct S;\n",
    ));
    let fixture = Fixture::new(&sources);
    let messages = unused_messages(&fixture.check(&["cc", "log", "serde"]));
    assert_eq!(
        messages,
        ["app declares unused dev-dependencies entry 'anyhow'"]
    );
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-008"))]
fn exceptions_skip_dependency_package_names() {
    let fixture = Fixture::new(&used_sources());
    let messages = unused_messages(&fixture.check(&["cc", "log", "serde"]));
    assert!(messages.is_empty(), "{messages:?}");
    let messages = unused_messages(&fixture.check(&["cc", "log", "serde", "json"]));
    assert!(messages.is_empty(), "rename keys are not package names");
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-009"))]
fn diagnostics_identify_manifest_line_and_crate_name() {
    let fixture = Fixture::new(&used_sources());
    let diagnostics = fixture.check(&["cc", "log"]);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(
        location(&diagnostics[0]),
        (Path::new("app/Cargo.toml"), Some(8))
    );
    assert!(diagnostics[0].notes[0].contains("`serde`"));
}

#[test]
#[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-010"))]
fn unreadable_sources_fail_without_unused_guesses() {
    let mut sources = used_sources();
    sources.push(("app/src/nested.rs", "pub fn broken( {\n"));
    let fixture = Fixture::new(&sources);
    let diagnostics = fixture.check(&[]);
    assert!(unused_messages(&diagnostics).is_empty(), "{diagnostics:?}");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "UnusedDependencySourceError");
    assert_eq!(location(&diagnostics[0]).0, Path::new("app/src/nested.rs"));
}
