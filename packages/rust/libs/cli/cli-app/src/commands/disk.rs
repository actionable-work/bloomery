use crate::emit_json;
use bloomery_cli_output::{self as output, Status, error_envelope, success_envelope};
use bloomery_disk::DiskReport;
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

pub(crate) fn run_disk_at(
    args: bloomery_cli_types::DiskArgs,
    json_mode: bool,
    color: output::ColorMode,
    root: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let backend = bloomery_disk::NixCli::default();
    run_disk_with_backend(args, json_mode, color, root, &backend, stdout, stderr)
}

pub(crate) fn run_disk_with_backend(
    args: bloomery_cli_types::DiskArgs,
    json_mode: bool,
    color: output::ColorMode,
    root: &Path,
    backend: &impl bloomery_disk::NixQuery,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let tree = args.tree;
    match bloomery_disk::run(root, &args, backend) {
        Ok(report) => {
            if json_mode {
                let mut document = disk_success_json(&report);
                if tree {
                    document["operation"] = serde_json::json!("tree");
                }
                if let Err(code) = emit_json(stdout, stderr, &document, "disk result") {
                    return code;
                }
            } else {
                let rendered = if tree {
                    bloomery_disk::render_text(&report)
                } else {
                    bloomery_disk::render_summary(&report)
                };
                let rendered = output::colorize_check(
                    &rendered,
                    output::color_enabled(output::Stream::Stdout, false, color),
                );
                let _ = stdout.write_all(rendered.as_bytes());
            }
            ExitCode::from(bloomery_disk::exit_code(&report))
        }
        Err(error) => {
            if json_mode {
                let document = match error.kind() {
                    bloomery_disk::DiskErrorKind::Usage => {
                        output::usage_error_json(Some("disk"), error.message())
                    }
                    bloomery_disk::DiskErrorKind::Operational => disk_failure_json(error.message()),
                };
                if let Err(code) = emit_json(stdout, stderr, &document, "disk error") {
                    return code;
                }
            } else {
                let _ = output::write_error(stderr, &error, color);
            }
            ExitCode::from(error.exit_code())
        }
    }
}

fn disk_success_json(report: &DiskReport) -> Value {
    success_envelope(
        "disk",
        None,
        Status::Succeeded,
        json!({
            "bytes": report.bytes,
            "systems": report.systems,
            "derivation": report.derivation,
            "unmeasured": report.unmeasured,
            "categories": report.categories,
        }),
        Vec::new(),
    )
}

fn disk_failure_json(message: &str) -> Value {
    error_envelope(
        Some("disk"),
        None,
        Status::Error,
        "DiskError",
        "disk",
        message,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::disk_success_json;
    use crate::commands::assert_no_ansi;
    use bloomery_disk::{DiskCategory, DiskEntry, DiskReport};

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-013"))]
    fn disk_json_reports_integer_bytes_and_unmeasured_targets() {
        let report = DiskReport {
            bytes: 4096,
            systems: vec!["x86_64-linux".to_owned()],
            derivation: None,
            unmeasured: vec!["/nix/store/aaa-check".to_owned()],
            categories: Vec::new(),
        };
        let document = disk_success_json(&report);
        assert_eq!(document["command"], "disk");
        assert_eq!(document["status"], "succeeded");
        assert_eq!(document["result"]["bytes"], 4096);
        assert_eq!(document["result"]["systems"][0], "x86_64-linux");
        assert_eq!(document["result"]["unmeasured"][0], "/nix/store/aaa-check");
        assert!(document["result"]["bytes"].is_u64());
        assert_no_ansi(&document);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-014"))]
    fn disk_json_reports_the_category_tree() {
        let report = DiskReport {
            bytes: 300,
            systems: vec!["x86_64-linux".to_owned()],
            derivation: None,
            unmeasured: Vec::new(),
            categories: vec![
                DiskCategory {
                    name: "packages".to_owned(),
                    bytes: 100,
                    entries: vec![DiskEntry {
                        name: "default".to_owned(),
                        bytes: 100,
                        unmeasured: false,
                    }],
                },
                DiskCategory {
                    name: "checks".to_owned(),
                    bytes: 200,
                    entries: vec![DiskEntry {
                        name: "app:test".to_owned(),
                        bytes: 200,
                        unmeasured: false,
                    }],
                },
            ],
        };
        let document = disk_success_json(&report);
        assert_eq!(document["result"]["categories"][0]["name"], "packages");
        assert_eq!(
            document["result"]["categories"][0]["entries"][0]["name"],
            "default"
        );
        assert_eq!(
            document["result"]["categories"][0]["entries"][0]["bytes"],
            100
        );
        assert_eq!(
            document["result"]["categories"][0]["entries"][0]["unmeasured"],
            false
        );
        assert_eq!(document["result"]["categories"][1]["name"], "checks");
    }
}
