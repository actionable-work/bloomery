#![allow(clippy::result_large_err)]

mod files;
mod nix;
mod rust;
mod typescript;

use bloomery_model::{Context, Diagnostic, Evidence};

pub fn scan_all(context: &Context) -> Result<Vec<Evidence>, Vec<Diagnostic>> {
    let mut evidence = Vec::new();
    let mut diagnostics = Vec::new();
    let scanners = &context.config.scanners;
    if scanners.rust.enabled {
        match rust::scan(&context.root, &scanners.rust.paths) {
            Ok(found) => evidence.extend(found),
            Err(errors) => diagnostics.extend(errors),
        }
    }
    if scanners.playwright.enabled {
        match typescript::scan(
            &context.root,
            &scanners.playwright.paths,
            &scanners.playwright.tag_prefix,
        ) {
            Ok(found) => evidence.extend(found),
            Err(errors) => diagnostics.extend(errors),
        }
    }
    if scanners.nix.enabled {
        match nix::scan(&context.root, &scanners.nix) {
            Ok(found) => evidence.extend(found),
            Err(errors) => diagnostics.extend(errors),
        }
    }
    if diagnostics.is_empty() {
        Ok(evidence)
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::scan_all;
    use bloomery_model::config::{
        NixScannerConfig, PlaywrightScannerConfig, RustScannerConfig, ScannersConfig, SpecsConfig,
    };
    use bloomery_model::{Config, Context};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-STATIC-001"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-STATIC-002"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-STATIC-003"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-STATIC-004"))]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-STATIC-005"))]
    fn extracts_static_references_with_locations_without_running_tests() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("bloomery-scanning-{suffix}"));
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
        let context = Context {
            root: root.clone(),
            config: Config {
                specs: SpecsConfig {
                    dir: "specs".into(),
                },
                scanners: ScannersConfig {
                    rust: RustScannerConfig {
                        enabled: true,
                        paths: vec!["tests/**/*.rs".into()],
                    },
                    playwright: PlaywrightScannerConfig {
                        enabled: true,
                        paths: vec!["e2e/**/*.spec.ts".into()],
                        tag_prefix: "@bloomery:".into(),
                    },
                    nix: NixScannerConfig {
                        enabled: true,
                        paths: vec!["checks.nix".into()],
                    },
                },
                ..Config::default()
            },
            areas: Vec::new(),
        };
        let evidence = scan_all(&context).expect("scanners should succeed");
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
}
