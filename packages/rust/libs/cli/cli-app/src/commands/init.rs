use crate::emit_json;
use bloomery_cli_output::{self as output, Status, error_envelope, success_envelope};
use bloomery_init::{InitError, InitErrorKind, InitReport};
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

pub(crate) fn run_init_at(
    args: bloomery_cli_types::InitArgs,
    json_mode: bool,
    color: output::ColorMode,
    root: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let directory = match args.directory {
        Some(path) if path.is_absolute() => path,
        Some(path) => root.join(path),
        None => root.to_path_buf(),
    };
    let request = bloomery_init::InitRequest {
        directory,
        template: args.template,
        force: args.force,
    };
    let mut stage = bloomery_init::SyncLockStage::system();
    run_init_request(&request, &mut stage, json_mode, color, stdout, stderr)
}

pub(crate) fn run_init_request<L: bloomery_init::LockStage>(
    request: &bloomery_init::InitRequest,
    stage: &mut L,
    json_mode: bool,
    color: output::ColorMode,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let mut progress = Vec::new();
    let mut progress_err = Vec::new();
    match bloomery_init::run(request, stage, &mut progress, &mut progress_err) {
        Ok(report) => {
            if json_mode {
                if let Err(code) =
                    emit_json(stdout, stderr, &init_success_json(&report), "init result")
                {
                    return code;
                }
            } else {
                let styled = output::color_enabled(output::Stream::Stdout, false, color);
                let _ = writeln!(stdout, "{}", output::heading("bloomery init", styled));
                for path in &report.created {
                    let _ = writeln!(stdout, "created {}", path.display());
                }
                let _ = writeln!(
                    stdout,
                    "Initialized '{}' from the '{}' template.",
                    report.directory.display(),
                    report.template
                );
                let _ = writeln!(
                    stdout,
                    "Generated lockfiles: {}.",
                    report.lockfiles.join(", ")
                );
                let _ = writeln!(stdout, "Next: run `nix develop` in the project.");
                let _ = stdout.flush();
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            if json_mode {
                if let Err(code) =
                    emit_json(stdout, stderr, &init_failure_json(&error), "init error")
                {
                    return code;
                }
            } else {
                let _ = stderr.write_all(&progress_err);
                let _ = output::write_error(stderr, &error, color);
            }
            ExitCode::from(error.exit_code())
        }
    }
}

fn init_success_json(report: &InitReport) -> Value {
    success_envelope(
        "init",
        None,
        Status::Succeeded,
        json!({
            "directory": report.directory,
            "template": report.template,
            "created": report.created,
            "lockfiles": report.lockfiles,
        }),
        Vec::new(),
    )
}

fn init_failure_json(error: &InitError) -> Value {
    let (code, kind) = match error.kind() {
        InitErrorKind::Usage => ("UsageError", "usage"),
        InitErrorKind::Operational => ("InitError", "operational"),
    };
    error_envelope(
        Some("init"),
        None,
        Status::Error,
        code,
        kind,
        error.message(),
        None,
    )
}
