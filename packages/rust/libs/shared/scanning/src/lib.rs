#![allow(clippy::result_large_err)]

mod files;
mod nix;
mod rust;
mod typescript;

use bloomery_model::{Context, Diagnostic, Evidence, TestSite};

/// One scanner pass: requirement references plus discovered test sites.
#[derive(Debug, Default)]
pub struct ScanOutput {
    pub evidence: Vec<Evidence>,
    pub tests: Vec<TestSite>,
}

impl ScanOutput {
    fn absorb(&mut self, other: ScanOutput) {
        self.evidence.extend(other.evidence);
        self.tests.extend(other.tests);
    }
}

pub fn scan_all(context: &Context) -> Result<ScanOutput, Vec<Diagnostic>> {
    let mut output = ScanOutput::default();
    let mut diagnostics = Vec::new();
    let scanners = &context.config.scanners;
    if scanners.rust.enabled {
        match rust::scan(&context.root, &scanners.rust.paths) {
            Ok(found) => output.absorb(found),
            Err(errors) => diagnostics.extend(errors),
        }
    }
    if scanners.playwright.enabled {
        match typescript::scan(
            &context.root,
            &scanners.playwright.paths,
            &scanners.playwright.tag_prefix,
        ) {
            Ok(found) => output.absorb(found),
            Err(errors) => diagnostics.extend(errors),
        }
    }
    if scanners.nix.enabled {
        match nix::scan(&context.root, &scanners.nix) {
            Ok(found) => output.absorb(found),
            Err(errors) => diagnostics.extend(errors),
        }
    }
    if diagnostics.is_empty() {
        Ok(output)
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::scan_all;
    use crate::{nix, rust, typescript};
    use bloomery_model::config::{
        NixScannerConfig, PlaywrightScannerConfig, RustScannerConfig, ScannersConfig, SpecsConfig,
    };
    use bloomery_model::{Config, Context, TestSite};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::sync::atomic::Ordering;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Serializes tests that read the process-wide parse counters so counts are
    /// not perturbed by other tests running in parallel.
    static SCAN_LOCK: Mutex<()> = Mutex::new(());

    fn root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("bloomery-scanning-{name}-{suffix}"))
    }

    fn context(
        root: &Path,
        rust_paths: &[&str],
        playwright_paths: &[&str],
        nix_paths: &[&str],
        nix_test_paths: &[&str],
    ) -> Context {
        Context {
            root: root.to_path_buf(),
            config: Config {
                specs: SpecsConfig {
                    dir: "specs".into(),
                },
                scanners: ScannersConfig {
                    rust: RustScannerConfig {
                        enabled: !rust_paths.is_empty(),
                        paths: rust_paths.iter().map(|path| (*path).to_owned()).collect(),
                    },
                    playwright: PlaywrightScannerConfig {
                        enabled: !playwright_paths.is_empty(),
                        paths: playwright_paths
                            .iter()
                            .map(|path| (*path).to_owned())
                            .collect(),
                        tag_prefix: "@bloomery:".into(),
                    },
                    nix: NixScannerConfig {
                        enabled: !nix_paths.is_empty() || !nix_test_paths.is_empty(),
                        paths: nix_paths.iter().map(|path| (*path).to_owned()).collect(),
                        test_paths: nix_test_paths
                            .iter()
                            .map(|path| (*path).to_owned())
                            .collect(),
                    },
                },
                ..Config::default()
            },
            areas: Vec::new(),
        }
    }

    fn site<'a>(tests: &'a [TestSite], name: &str) -> &'a TestSite {
        tests
            .iter()
            .find(|site| site.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("test site '{name}' should be discovered"))
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-STATIC-001"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-STATIC-002"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-STATIC-003"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-STATIC-004"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-STATIC-005"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-DISCOVERY-009"))]
    fn extracts_static_references_with_locations_without_running_tests() {
        let _guard = SCAN_LOCK.lock().expect("scan lock");
        let root = root("static");
        fs::create_dir_all(root.join("tests")).expect("tests");
        fs::create_dir_all(root.join("e2e")).expect("e2e");
        fs::write(
            root.join("tests/example.rs"),
            "#[cfg_attr(any(), bloomery(\"PARSER-SCANNING-TESTS-001\"))]\n#[test]\nfn example() { panic!(\"scanner must not execute source tests\") }\n",
        )
        .expect("Rust source");
        fs::write(
            root.join("e2e/example.spec.ts"),
            "test('example', { tag: ['@bloomery:PARSER-SCANNING-TESTS-002'] }, async () => { throw new Error('scanner must not execute source tests'); });\n",
        )
        .expect("Playwright source");
        fs::write(
            root.join("checks.nix"),
            r#"# passthru.bloomery = [ "PARSER-SCANNING-TESTS-999" ];
let decoy = "passthru.bloomery = [ PARSER-SCANNING-TESTS-998 ];";
in builtins.seq (builtins.abort "scanner must not evaluate Nix") {
  nested = {
    passthru = {
      bloomery = [
        "PARSER-SCANNING-TESTS-003"
      ];
    };
  };
  direct = {
    passthru.bloomery = lib.optionals (name == "basic-workspace") [
      "PARSER-SCANNING-TESTS-004"
    ];
  };
  indented = ''
    passthru.bloomery = [ "PARSER-SCANNING-TESTS-997" ];
    ''${notMetadata}
  '';
}
"#,
        )
        .expect("Nix source");
        let context = context(
            &root,
            &["tests/**/*.rs"],
            &["e2e/**/*.spec.ts"],
            &["checks.nix"],
            &[],
        );
        let output = scan_all(&context).expect("scanners should succeed");
        let evidence = &output.evidence;
        assert_eq!(evidence.len(), 4);
        let rust = evidence
            .iter()
            .find(|item| item.id == "PARSER-SCANNING-TESTS-001")
            .expect("Rust test reference");
        assert_eq!(rust.scanner, "rust");
        assert_eq!(rust.location.path, root.join("tests/example.rs"));
        assert_eq!(rust.location.line, Some(1));

        let playwright = evidence
            .iter()
            .find(|item| item.id == "PARSER-SCANNING-TESTS-002")
            .expect("Playwright test reference");
        assert_eq!(playwright.scanner, "playwright");
        assert_eq!(playwright.location.path, root.join("e2e/example.spec.ts"));
        assert_eq!(playwright.location.line, Some(1));

        let nix_nested = evidence
            .iter()
            .find(|item| item.id == "PARSER-SCANNING-TESTS-003")
            .expect("nested Nix metadata reference");
        assert_eq!(nix_nested.scanner, "nix");
        assert_eq!(nix_nested.location.path, root.join("checks.nix"));
        assert_eq!(nix_nested.location.line, Some(7));

        let nix_direct = evidence
            .iter()
            .find(|item| item.id == "PARSER-SCANNING-TESTS-004")
            .expect("direct Nix metadata reference");
        assert_eq!(nix_direct.scanner, "nix");
        assert_eq!(nix_direct.location.path, root.join("checks.nix"));
        assert_eq!(nix_direct.location.line, Some(13));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-DISCOVERY-001"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-DISCOVERY-003"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-DISCOVERY-004"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-DISCOVERY-006"))]
    fn rust_discovery_creates_test_sites() {
        let root = root("rust-discovery");
        fs::create_dir_all(root.join("tests")).expect("tests");
        fs::write(
            root.join("tests/a.rs"),
            "#[cfg_attr(any(), bloomery(\"PARSER-SCANNING-DISC-001\"))]\n#[test]\nfn tagged() {}\n\n#[test]\nfn untagged() {}\n\n#[cfg_attr(any(), bloomery(\"PARSER-SCANNING-DISC-002\"))]\nfn not_a_test() {}\n",
        )
        .expect("Rust source");
        let context = context(&root, &["tests/**/*.rs"], &[], &[], &[]);
        let output = scan_all(&context).expect("scan");
        assert_eq!(output.tests.len(), 2, "only test functions are sites");
        let tagged = site(&output.tests, "tagged");
        assert_eq!(tagged.scanner, "rust");
        assert_eq!(tagged.references, ["PARSER-SCANNING-DISC-001"]);
        assert_eq!(tagged.location.path, root.join("tests/a.rs"));
        let untagged = site(&output.tests, "untagged");
        assert!(untagged.references.is_empty());
        assert!(
            !output
                .tests
                .iter()
                .any(|site| site.name.as_deref() == Some("not_a_test"))
        );
        assert!(
            output
                .evidence
                .iter()
                .all(|item| item.id != "PARSER-SCANNING-DISC-002"),
            "non-test functions contribute no evidence"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-DISCOVERY-002"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-DISCOVERY-003"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-DISCOVERY-004"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-DISCOVERY-007"))]
    fn playwright_discovery_creates_test_sites() {
        let root = root("playwright-discovery");
        fs::create_dir_all(root.join("e2e")).expect("e2e");
        fs::write(
            root.join("e2e/a.spec.ts"),
            "test('tagged', { tag: ['@bloomery:PARSER-SCANNING-DISC-003'] }, async () => {});\ntest('untagged', async () => {});\nnotATest('ignored', { tag: ['@bloomery:PARSER-SCANNING-DISC-004'] }, async () => {});\n",
        )
        .expect("Playwright source");
        let context = context(&root, &[], &["e2e/**/*.spec.ts"], &[], &[]);
        let output = scan_all(&context).expect("scan");
        assert_eq!(output.tests.len(), 2);
        assert_eq!(
            site(&output.tests, "tagged").references,
            ["PARSER-SCANNING-DISC-003"]
        );
        assert!(site(&output.tests, "untagged").references.is_empty());
        assert!(
            !output
                .tests
                .iter()
                .any(|site| site.name.as_deref() == Some("ignored"))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-DISCOVERY-008"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-DISCOVERY-011"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-DISCOVERY-012"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-DISCOVERY-013"))]
    fn nix_test_paths_discover_sites() {
        let root = root("nix-discovery");
        fs::create_dir_all(root.join("lib")).expect("lib");
        fs::write(
            root.join("lib/tagged.test.nix"),
            "{\n  passthru.bloomery = [ \"PARSER-SCANNING-DISC-005\" ];\n}\n",
        )
        .expect("tagged Nix test");
        fs::write(root.join("lib/untagged.test.nix"), "{ }\n").expect("untagged Nix test");
        fs::write(
            root.join("checks.nix"),
            "{\n  passthru.bloomery = [ \"PARSER-SCANNING-DISC-006\" ];\n}\n",
        )
        .expect("Nix check");
        let context = context(&root, &[], &[], &["*.nix"], &["**/*.test.nix"]);
        let output = scan_all(&context).expect("scan");
        assert_eq!(output.tests.len(), 2);
        assert_eq!(
            site(&output.tests, "tagged.test").references,
            ["PARSER-SCANNING-DISC-005"]
        );
        assert!(site(&output.tests, "untagged.test").references.is_empty());
        assert!(
            output
                .evidence
                .iter()
                .any(|item| item.id == "PARSER-SCANNING-DISC-006")
        );
        assert!(
            output
                .evidence
                .iter()
                .any(|item| item.id == "PARSER-SCANNING-DISC-005")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-DISCOVERY-005"))]
    fn discovery_is_keyed_by_source_location() {
        let root = root("location-dedup");
        fs::create_dir_all(root.join("tests")).expect("tests");
        fs::write(root.join("tests/once.rs"), "#[test]\nfn once() {}\n").expect("Rust source");
        let context = context(&root, &["tests/**/*.rs", "tests/once.rs"], &[], &[], &[]);
        let output = scan_all(&context).expect("scan");
        assert_eq!(output.tests.len(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-DISCOVERY-010"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-PARSING-005"))]
    fn unparsable_file_reports_one_diagnostic() {
        let _guard = SCAN_LOCK.lock().expect("scan lock");
        let root = root("parse-error");
        fs::create_dir_all(root.join("tests")).expect("tests");
        fs::write(root.join("tests/bad.rs"), "fn broken( {").expect("bad source");
        let context = context(&root, &["tests/**/*.rs", "tests/bad.rs"], &[], &[], &[]);
        let errors = scan_all(&context).expect_err("parse error");
        assert_eq!(errors.len(), 1, "one diagnostic per file per command");
        assert_eq!(errors[0].code, "RustParseError");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-PARSING-001"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-PARSING-002"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-PARSING-003"))]
    fn single_parse_serves_references_and_discovery() {
        let _guard = SCAN_LOCK.lock().expect("scan lock");
        let root = root("parse-once");
        fs::create_dir_all(root.join("tests")).expect("tests");
        fs::write(
            root.join("tests/once.rs"),
            "#[cfg_attr(any(), bloomery(\"PARSER-SCANNING-DISC-007\"))]\n#[test]\nfn once() {}\n",
        )
        .expect("Rust source");
        let context = context(&root, &["tests/**/*.rs", "tests/once.rs"], &[], &[], &[]);
        let before = rust::PARSE_COUNT.load(Ordering::Relaxed);
        let output = scan_all(&context).expect("scan");
        let parsed = rust::PARSE_COUNT.load(Ordering::Relaxed) - before;
        assert_eq!(parsed, 1, "overlapping globs and both roles parse once");
        assert_eq!(output.tests.len(), 1);
        assert_eq!(output.evidence.len(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-PARSING-001"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-PARSING-003"))]
    fn nix_union_of_paths_and_test_paths_parses_once() {
        let _guard = SCAN_LOCK.lock().expect("scan lock");
        let root = root("nix-parse-once");
        fs::create_dir_all(root.join("lib")).expect("lib");
        fs::write(
            root.join("lib/union.test.nix"),
            "{\n  passthru.bloomery = [ \"PARSER-SCANNING-DISC-008\" ];\n}\n",
        )
        .expect("Nix test");
        let context = context(&root, &[], &[], &["lib/**/*.nix"], &["**/*.test.nix"]);
        let before = nix::PARSE_COUNT.load(Ordering::Relaxed);
        let output = scan_all(&context).expect("scan");
        let parsed = nix::PARSE_COUNT.load(Ordering::Relaxed) - before;
        assert_eq!(parsed, 1, "union scans a file once");
        assert_eq!(output.tests.len(), 1);
        assert_eq!(output.evidence.len(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-PARSING-004"))]
    fn parse_cache_is_command_scoped() {
        let _guard = SCAN_LOCK.lock().expect("scan lock");
        let root = root("cache-scope");
        fs::create_dir_all(root.join("tests")).expect("tests");
        fs::write(root.join("tests/once.rs"), "#[test]\nfn once() {}\n").expect("Rust source");
        let context = context(&root, &["tests/**/*.rs"], &[], &[], &[]);
        let before = rust::PARSE_COUNT.load(Ordering::Relaxed);
        scan_all(&context).expect("first scan");
        let first = rust::PARSE_COUNT.load(Ordering::Relaxed) - before;
        scan_all(&context).expect("second scan");
        let second = rust::PARSE_COUNT.load(Ordering::Relaxed) - before - first;
        assert_eq!(first, 1);
        assert_eq!(second, 1, "the cache does not persist across commands");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-DISCOVERY-003"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-DISCOVERY-004"))]
    fn playwright_parse_count_is_per_file() {
        let _guard = SCAN_LOCK.lock().expect("scan lock");
        let root = root("playwright-once");
        fs::create_dir_all(root.join("e2e")).expect("e2e");
        fs::write(root.join("e2e/a.spec.ts"), "test('a', async () => {});\n").expect("source");
        let context = context(&root, &[], &["e2e/**/*.spec.ts", "e2e/a.spec.ts"], &[], &[]);
        let before = typescript::PARSE_COUNT.load(Ordering::Relaxed);
        scan_all(&context).expect("scan");
        let parsed = typescript::PARSE_COUNT.load(Ordering::Relaxed) - before;
        assert_eq!(parsed, 1);
        let _ = fs::remove_dir_all(root);
    }
}
