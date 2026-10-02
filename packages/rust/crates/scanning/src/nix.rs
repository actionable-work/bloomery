use bloomery_model::{Diagnostic, Evidence, SourceLocation, config::NixScannerConfig};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

pub fn scan(root: &Path, config: &NixScannerConfig) -> Result<Vec<Evidence>, Vec<Diagnostic>> {
    let systems = if config.systems.is_empty() {
        vec![host_system()]
    } else {
        config.systems.clone()
    };
    let mut evidence = Vec::new();
    let mut diagnostics = Vec::new();
    for system in systems {
        let attribute = format!("{}.{}", config.checks_attr.trim_end_matches('.'), system);
        let output = Command::new("nix")
            .current_dir(root)
            .args([
                "eval",
                &attribute,
                "--json",
                "--apply",
                "builtins.mapAttrs (name: drv: drv.bloomery or [])",
            ])
            .output();
        let output = match output {
            Ok(output) => output,
            Err(error) => {
                diagnostics.push(
                    Diagnostic::new(
                        "NixScanError",
                        format!("Unable to invoke nix eval: {error}"),
                    )
                    .at(root.join("flake.nix"), Some(1)),
                );
                continue;
            }
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            diagnostics.push(
                Diagnostic::new(
                    "NixScanError",
                    format!("nix eval failed for {attribute}: {}", stderr.trim()),
                )
                .at(root.join("flake.nix"), Some(1)),
            );
            continue;
        }
        let json: Value = match serde_json::from_slice(&output.stdout) {
            Ok(json) => json,
            Err(error) => {
                diagnostics.push(
                    Diagnostic::new(
                        "NixScanError",
                        format!("nix eval returned invalid JSON: {error}"),
                    )
                    .at(root.join("flake.nix"), Some(1)),
                );
                continue;
            }
        };
        let Some(checks) = json.as_object() else {
            diagnostics.push(
                Diagnostic::new(
                    "NixScanError",
                    "nix checks evaluation did not return an object",
                )
                .at(root.join("flake.nix"), Some(1)),
            );
            continue;
        };
        for (check, ids) in checks {
            let Some(ids) = ids.as_array() else {
                diagnostics.push(
                    Diagnostic::new(
                        "NixScanError",
                        format!("Check '{check}' bloomery metadata is not an array"),
                    )
                    .at(root.join("flake.nix"), Some(1)),
                );
                continue;
            };
            for id in ids {
                let Some(id) = id.as_str() else {
                    diagnostics.push(
                        Diagnostic::new(
                            "NixScanError",
                            format!("Check '{check}' contains a non-string requirement ID"),
                        )
                        .at(root.join("flake.nix"), Some(1)),
                    );
                    continue;
                };
                evidence.push(Evidence {
                    id: id.to_owned(),
                    location: SourceLocation::new(root.join("flake.nix"), None),
                    scanner: "nix",
                });
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(evidence)
    } else {
        Err(diagnostics)
    }
}

fn host_system() -> String {
    let operating_system = match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    };
    format!("{}-{operating_system}", std::env::consts::ARCH)
}
