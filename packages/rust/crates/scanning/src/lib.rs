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
    fn extracts_rust_and_playwright_references() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("bloomery-scanning-{suffix}"));
        fs::create_dir_all(root.join("tests")).expect("tests");
        fs::create_dir_all(root.join("e2e")).expect("e2e");
        fs::write(
            root.join("tests/example.rs"),
            "#[bloomery(\"PARSER-SCANNING-TESTS-001\")]\n#[test]\nfn example() {}\n",
        )
        .expect("Rust source");
        fs::write(
            root.join("e2e/example.spec.ts"),
            "test('example', { tag: ['@bloomery:PARSER-SCANNING-TESTS-002'] }, async () => {});\n",
        )
        .expect("Playwright source");
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
                    nix: NixScannerConfig::default(),
                },
            },
            areas: Vec::new(),
        };
        let evidence = scan_all(&context).expect("scanners should succeed");
        assert_eq!(evidence.len(), 2);
        assert!(
            evidence
                .iter()
                .any(|item| item.id == "PARSER-SCANNING-TESTS-001")
        );
        assert!(
            evidence
                .iter()
                .any(|item| item.id == "PARSER-SCANNING-TESTS-002")
        );
        let _ = fs::remove_dir_all(root);
    }
}
