use crate::{emit_json, write_diagnostics};
use bloomery_cli_output::{self as output, Status, Warning, error_envelope, success_envelope};
use bloomery_cli_types::flake_present;
use bloomery_config::{
    ConfigError, ConfigErrorKind, ConfigOutcome, ConfigReport, ConfigWarning, DiffStatus,
};
use serde_json::Value;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

pub(crate) fn run_config_at(
    args: bloomery_cli_types::ConfigArgs,
    json_mode: bool,
    color: output::ColorMode,
    root: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let mut boundary = Vec::new();
    if !flake_present(root) {
        boundary.push(ConfigWarning::MissingFlake);
    }
    match bloomery_config::run(root, &args.operation) {
        Ok(mut report) => {
            report.prepend_warnings(boundary);
            if json_mode {
                if let Err(code) = emit_json(
                    stdout,
                    stderr,
                    &config_success_json(&report),
                    "config result",
                ) {
                    return code;
                }
            } else {
                render_config_warnings(&report.warnings, color, stderr);
                render_config_outcome(&report, color, stdout);
                let _ = stdout.flush();
            }
            ExitCode::SUCCESS
        }
        Err(error) => report_config_error(error, root, json_mode, stdout, stderr),
    }
}

fn render_config_warnings(
    warnings: &[ConfigWarning],
    color: output::ColorMode,
    stderr: &mut impl Write,
) {
    let styled = output::color_enabled(output::Stream::Stderr, false, color);
    for warning in warnings {
        let message = match warning {
            ConfigWarning::MissingFlake => {
                "warning: no Bloomery flake.nix in the current directory".to_owned()
            }
            ConfigWarning::MissingConfig { path } => {
                format!("warning: {path} is missing; reporting documented defaults")
            }
            ConfigWarning::CreatedConfig { path } => format!("warning: created {path}"),
        };
        let _ = writeln!(
            stderr,
            "{}",
            output::style(output::Role::Warning, &message, styled)
        );
    }
}

fn render_config_value(value: Option<&toml::Value>) -> String {
    value
        .map(ToString::to_string)
        .unwrap_or_else(|| "unset".to_owned())
}

pub(crate) fn render_config_outcome(
    report: &ConfigReport,
    color: output::ColorMode,
    stdout: &mut impl Write,
) {
    let styled = output::color_enabled(output::Stream::Stdout, false, color);
    let _ = writeln!(stdout, "{}", output::heading("bloomery config", styled));
    match &report.outcome {
        ConfigOutcome::Get { key } => {
            let origin = if key.configured {
                "configured"
            } else if key.recommended {
                "default"
            } else {
                "unset"
            };
            let _ = writeln!(
                stdout,
                "{} = {} ({origin})",
                key.key,
                render_config_value(key.value.as_ref())
            );
        }
        ConfigOutcome::List { keys } => {
            for key in keys {
                let origin = if key.configured {
                    "configured"
                } else if key.recommended {
                    "default"
                } else {
                    "unset"
                };
                let _ = writeln!(
                    stdout,
                    "{} = {} ({origin})",
                    key.key,
                    render_config_value(key.value.as_ref())
                );
            }
        }
        ConfigOutcome::Set {
            key,
            value,
            changed,
        } => {
            if *changed {
                let _ = writeln!(stdout, "set {key} = {value}");
            } else {
                let _ = writeln!(stdout, "{key} unchanged");
            }
        }
        ConfigOutcome::Unset {
            key,
            removed,
            changed,
        } => {
            if *removed {
                let _ = writeln!(stdout, "unset {key}");
            } else if *changed {
                let _ = writeln!(stdout, "created configuration");
            } else {
                let _ = writeln!(stdout, "{key} absent");
            }
        }
        ConfigOutcome::Upgrade {
            source_version,
            target_version,
            added,
            written,
            differences,
        } => {
            if let (Some(source), Some(target)) = (source_version, target_version) {
                let _ = writeln!(stdout, "format migration: {source} -> {target}");
            }
            if !differences.is_empty() {
                for record in differences {
                    let status = match record.status {
                        DiffStatus::Equal => "equal",
                        DiffStatus::Differing => "differs",
                        DiffStatus::Absent => "absent",
                        DiffStatus::Unset => "unset",
                    };
                    let _ = writeln!(
                        stdout,
                        "{}: {status} (recommended {}, current {})",
                        record.key,
                        render_config_value(record.recommended.as_ref()),
                        render_config_value(record.current.as_ref())
                    );
                }
            } else if added.is_empty() {
                let _ = writeln!(stdout, "configuration already current");
            } else {
                for key in added {
                    let _ = writeln!(stdout, "added {key}");
                }
                if !written {
                    let _ = writeln!(stdout, "(dry run; not written)");
                }
            }
        }
        ConfigOutcome::Document {
            documented,
            written,
        } => {
            if documented.is_empty() {
                let _ = writeln!(stdout, "no keys documented");
            } else {
                for key in documented {
                    let _ = writeln!(stdout, "documented {key}");
                }
            }
            if !written {
                let _ = writeln!(stdout, "(not written)");
            }
        }
    }
}

fn report_config_error(
    error: ConfigError,
    root: &Path,
    json_mode: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let exit_code = error.exit_code();
    if json_mode {
        let document = match error.diagnostic() {
            Some(diagnostic) => {
                output::diagnostics_json("config", "failed", root, std::slice::from_ref(diagnostic))
            }
            None if error.kind() == ConfigErrorKind::Usage => {
                output::usage_error_json(Some("config"), error.message())
            }
            None => config_failure_json("config", error.message()),
        };
        if let Err(code) = emit_json(stdout, stderr, &document, "config error") {
            return code;
        }
    } else {
        match error.diagnostic() {
            Some(diagnostic) => {
                write_diagnostics(
                    stderr,
                    root,
                    std::slice::from_ref(diagnostic),
                    output::ColorMode::Auto,
                );
            }
            None => {
                let _ = output::write_error(stderr, error.message(), output::ColorMode::Auto);
            }
        }
    }
    ExitCode::from(exit_code)
}

pub(crate) fn config_success_json(report: &ConfigReport) -> Value {
    let result = serde_json::to_value(&report.outcome).unwrap_or(Value::Null);
    success_envelope(
        "config",
        None,
        Status::Succeeded,
        result,
        config_warnings(&report.warnings),
    )
}

fn config_failure_json(command: &str, message: &str) -> Value {
    error_envelope(
        Some(command),
        None,
        Status::Error,
        "ConfigError",
        "config",
        message,
        None,
    )
}

fn config_warnings(warnings: &[bloomery_config::ConfigWarning]) -> Vec<Warning> {
    warnings
        .iter()
        .map(|warning| match warning {
            bloomery_config::ConfigWarning::MissingFlake => Warning::new(
                "missing_flake",
                "no Bloomery flake.nix in the current directory",
            ),
            bloomery_config::ConfigWarning::MissingConfig { path } => {
                Warning::new("missing_config", format!("{path} is missing"))
            }
            bloomery_config::ConfigWarning::CreatedConfig { path } => {
                Warning::new("created_config", format!("created {path}"))
            }
        })
        .collect()
}
