use crate::{emit_json, load_configuration_preflight, report_missing_flake};
use bloomery_cli_output::{
    self as output, Role, Status, Warning, attach_warnings, error_envelope, style, success_envelope,
};
use bloomery_cli_types::flake_present;
use bloomery_sync::SyncReport;
use serde_json::{Value, json};
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

pub(crate) fn run_sync_at(
    root: &Path,
    update: Option<&str>,
    json_mode: bool,
    color: output::ColorMode,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    if !flake_present(root) {
        return report_missing_flake(Some("sync"), json_mode, stdout, stderr);
    }
    if let Err(code) = load_configuration_preflight(Some("sync"), root, json_mode, stdout, stderr) {
        return code;
    }
    let selection = match bloomery_sync::parse_cli_update(update) {
        Ok(selection) => selection,
        Err(error) => {
            if json_mode {
                let document = output::usage_error_json(Some("sync"), &error.to_string());
                if let Err(code) = emit_json(stdout, stderr, &document, "usage error") {
                    return code;
                }
            } else {
                let _ = output::write_error(
                    stderr,
                    format_args!("{error}\nUsage: bloomery sync [--update[=nix,rust]]"),
                    color,
                );
            }
            return ExitCode::from(error.exit_code());
        }
    };

    let mut runner = bloomery_sync::SystemCommandRunner;
    if json_mode {
        let mut progress = Vec::new();
        let mut diagnostics = Vec::new();
        return match bloomery_sync::run(
            root,
            &selection,
            &mut runner,
            &mut progress,
            &mut diagnostics,
        ) {
            Ok(report) => {
                match emit_json(stdout, stderr, &sync_success_json(&report), "sync result") {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(code) => code,
                }
            }
            Err(error) => {
                let exit_code = error.exit_code();
                let document = sync_failure_json(
                    error.stage(),
                    error.message(),
                    error.completed_stages(),
                    error.may_be_partially_synchronized(),
                    &String::from_utf8_lossy(&diagnostics),
                );
                match emit_json(stdout, stderr, &document, "sync error") {
                    Ok(()) => ExitCode::from(exit_code),
                    Err(code) => code,
                }
            }
        };
    }

    let result = {
        let mut stdout_writer = ColorWriter::stdout(
            &mut *stdout,
            output::color_enabled(output::Stream::Stdout, false, color),
        );
        let mut stderr_writer = ColorWriter::stderr(
            &mut *stderr,
            output::color_enabled(output::Stream::Stderr, false, color),
        );
        let result = bloomery_sync::run(
            root,
            &selection,
            &mut runner,
            &mut stdout_writer,
            &mut stderr_writer,
        );
        let _ = stdout_writer.flush();
        let _ = stderr_writer.flush();
        result
    };
    match result {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = output::write_error(stderr, &error, color);
            ExitCode::from(error.exit_code())
        }
    }
}

fn sync_success_json(report: &SyncReport) -> Value {
    success_envelope(
        "sync",
        None,
        Status::Succeeded,
        serde_json::to_value(report).unwrap_or(Value::Null),
        Vec::new(),
    )
}

fn sync_failure_json(
    stage: &str,
    message: &str,
    completed_stages: &[String],
    partially_synchronized: bool,
    captured_stderr: &str,
) -> Value {
    let (warnings, recommendations, tool_stderr) = categorize_stderr(captured_stderr);
    let notices = warnings
        .iter()
        .map(|warning| Warning::new("warning", warning.clone()))
        .chain(
            recommendations
                .iter()
                .map(|recommendation| Warning::new("recommendation", recommendation.clone())),
        )
        .collect::<Vec<_>>();
    let details = json!({
        "stage": stage,
        "completed_stages": completed_stages,
        "partially_synchronized": partially_synchronized,
        "recovery_guidance": if partially_synchronized {
            Some("Fix the error and rerun `bloomery sync`.")
        } else {
            None
        },
        "tool_stderr": tool_stderr,
    });
    let envelope = error_envelope(
        Some("sync"),
        None,
        Status::Error,
        "SyncFailed",
        "sync",
        message,
        Some(details),
    );
    attach_warnings(envelope, &notices)
}

fn categorize_stderr(contents: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut recommendations = Vec::new();
    let mut tool_stderr = Vec::new();
    for line in contents.lines() {
        if let Some(message) = line.strip_prefix("warning: ") {
            warnings.push(message.to_owned());
        } else if let Some(message) = line.strip_prefix("recommendation: ") {
            recommendations.push(message.to_owned());
        } else if !line.is_empty() {
            tool_stderr.push(line.to_owned());
        }
    }
    (warnings, recommendations, tool_stderr)
}

#[derive(Debug, Clone, Copy)]
enum Channel {
    Stdout,
    Stderr,
}

/// Line-buffering adapter that adds ANSI styles to the sync command's progress
/// and advisory lines without changing subprocess output.
struct ColorWriter<W> {
    inner: W,
    enabled: bool,
    channel: Channel,
    line: Vec<u8>,
}

impl<W> ColorWriter<W> {
    fn stdout(inner: W, enabled: bool) -> Self {
        Self {
            inner,
            enabled,
            channel: Channel::Stdout,
            line: Vec::new(),
        }
    }

    fn stderr(inner: W, enabled: bool) -> Self {
        Self {
            inner,
            enabled,
            channel: Channel::Stderr,
            line: Vec::new(),
        }
    }
}

impl<W: Write> ColorWriter<W> {
    fn emit_line(&mut self, newline: bool) -> io::Result<()> {
        let raw = String::from_utf8_lossy(&self.line);
        let line = raw.strip_suffix('\r').unwrap_or(&raw);
        let role = match self.channel {
            Channel::Stdout if line.starts_with("==>") => Some(Role::Accent),
            Channel::Stdout if line.starts_with("Successfully synchronized") => Some(Role::Success),
            Channel::Stdout if line.starts_with("Skipped ") => Some(Role::Warning),
            Channel::Stderr if line.starts_with("warning:") => Some(Role::Warning),
            Channel::Stderr if line.starts_with("recommendation:") => Some(Role::Info),
            _ => None,
        };
        if self.enabled {
            if let Some(role) = role {
                write!(self.inner, "{}", style(role, line, true))?;
            } else {
                self.inner.write_all(line.as_bytes())?;
            }
        } else {
            self.inner.write_all(line.as_bytes())?;
        }
        if newline {
            self.inner.write_all(b"\n")?;
        }
        self.line.clear();
        Ok(())
    }
}

impl<W: Write> Write for ColorWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut start = 0;
        for (index, byte) in buffer.iter().enumerate() {
            if *byte == b'\n' {
                self.line.extend_from_slice(&buffer[start..index]);
                self.emit_line(true)?;
                start = index + 1;
            }
        }
        self.line.extend_from_slice(&buffer[start..]);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.line.is_empty() {
            self.emit_line(false)?;
        }
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::{ColorWriter, sync_failure_json, sync_success_json};
    use crate::commands::assert_no_ansi;
    use bloomery_sync::SyncReport;
    use std::io::Write;

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-006"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-007"))]
    fn sync_json_contains_structured_completion_and_partial_failure_data() {
        let report = SyncReport {
            reconciled_locks: vec!["Cargo.lock".to_owned(), "bloomery.lock".to_owned()],
            updated_ecosystems: vec!["rust".to_owned()],
            skipped_updates: vec!["nix".to_owned()],
            completed_stages: vec!["Cargo dependency update".to_owned()],
            warnings: vec!["config missing".to_owned()],
            recommendations: Vec::new(),
            tool_stderr: Vec::new(),
        };
        let success = sync_success_json(&report);
        assert_eq!(success["command"], "sync");
        assert_eq!(success["status"], "succeeded");
        assert_eq!(success["result"]["updated_ecosystems"][0], "rust");
        assert_no_ansi(&success);

        let failure = sync_failure_json(
            "final Cargo metadata",
            "exit status 1",
            &["Cargo lock reconciliation".to_owned()],
            true,
            "warning: advisory\nrecommendation: feature\ntool detail\n",
        );
        assert_eq!(failure["error"]["details"]["stage"], "final Cargo metadata");
        assert_eq!(failure["error"]["details"]["partially_synchronized"], true);
        assert_eq!(
            failure["error"]["details"]["recovery_guidance"],
            "Fix the error and rerun `bloomery sync`."
        );
        assert_eq!(failure["warnings"][0]["message"], "advisory");
        assert_eq!(failure["warnings"][1]["message"], "feature");
        assert_eq!(failure["error"]["details"]["tool_stderr"][0], "tool detail");
        assert_no_ansi(&failure);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-001"))]
    fn sync_color_writer_styles_only_progress_and_advisories() {
        let mut output = Vec::new();
        {
            let mut writer = ColorWriter::stdout(&mut output, true);
            writer
                .write_all(b"==> Stage\nSuccessfully synchronized locks\nplain subprocess output\n")
                .expect("write");
            writer.flush().expect("flush");
        }
        let output = String::from_utf8(output).expect("UTF-8");
        assert!(output.contains("\x1b[1;36m==> Stage\x1b[0m\n"));
        assert!(output.contains("\x1b[1;32mSuccessfully synchronized locks\x1b[0m\n"));
        assert!(output.contains("plain subprocess output\n"));

        let mut errors = Vec::new();
        {
            let mut writer = ColorWriter::stderr(&mut errors, true);
            writer
                .write_all(b"warning: config\nrecommendation: scanner\n")
                .expect("write notices");
            writer.flush().expect("flush notices");
        }
        let errors = String::from_utf8(errors).expect("UTF-8 stderr");
        assert!(errors.contains("\x1b[1;33mwarning: config\x1b[0m\n"));
        assert!(errors.contains("\x1b[36mrecommendation: scanner\x1b[0m\n"));
    }
}
