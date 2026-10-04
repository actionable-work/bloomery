mod terminal;

pub use terminal::{TerminalFacts, TransientLine};

use bloomery_model::{Diagnostic, diagnostics::SourceLocation};
use bloomery_sync::SyncReport;
use serde_json::{Value, json};
use std::io::{self, IsTerminal, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

pub fn color_enabled(stream: Stream, json_mode: bool) -> bool {
    let terminal = match stream {
        Stream::Stdout => io::stdout().is_terminal(),
        Stream::Stderr => io::stderr().is_terminal(),
    };
    color_policy(
        terminal,
        std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty()),
        std::env::var("TERM").is_ok_and(|term| term == "dumb"),
        json_mode,
    )
}

fn color_policy(terminal: bool, no_color: bool, dumb_terminal: bool, json_mode: bool) -> bool {
    terminal && !no_color && !dumb_terminal && !json_mode
}

pub fn diagnostics_json(
    command: &str,
    status: &str,
    root: &Path,
    diagnostics: &[Diagnostic],
) -> Value {
    let values = diagnostics
        .iter()
        .map(|diagnostic| diagnostic_json(root, diagnostic))
        .collect::<Vec<_>>();
    json!({
        "command": command,
        "status": status,
        "diagnostics": values,
    })
}

fn diagnostic_json(root: &Path, diagnostic: &Diagnostic) -> Value {
    let location = diagnostic.location.as_ref().map(|location| {
        json!({
            "path": relative_path(root, location),
            "line": location.line,
            "column": location.column,
        })
    });
    json!({
        "code": &diagnostic.code,
        "message": &diagnostic.message,
        "location": location,
        "notes": &diagnostic.notes,
    })
}

fn relative_path(root: &Path, location: &SourceLocation) -> String {
    location
        .display_path(root)
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

pub fn colorize_check(text: &str, enabled: bool) -> String {
    colorize_lines(text, enabled, |line| {
        if line.starts_with("PASS") {
            Some("1;32")
        } else if line.starts_with("ERROR [")
            || line.starts_with("ERROR  ")
            || line.starts_with("FAIL")
            || line.starts_with("INTERRUPTED")
        {
            Some("1;31")
        } else if line.starts_with("  -->") {
            Some("36")
        } else if line.starts_with("  = note:") {
            Some("33")
        } else if is_failure_id_line(line) {
            Some("1;31")
        } else {
            None
        }
    })
}

pub fn colorize_review(text: &str, enabled: bool) -> String {
    colorize_lines(text, enabled, |line| {
        if line.starts_with("Total manual requirements") {
            Some("1;32")
        } else if line.starts_with("            Design:") {
            Some("36")
        } else if line.starts_with("        └── [") {
            Some("1;35")
        } else if line.starts_with("    └──") || line.starts_with("└──") {
            Some("1;36")
        } else if !line.trim().is_empty() && !line.starts_with(' ') {
            Some("1;34")
        } else {
            None
        }
    })
}

pub fn colorize_error(text: &str, enabled: bool) -> String {
    colorize_lines(text, enabled, |line| {
        if line.starts_with("error:") || line.starts_with("sync failed") {
            Some("1;31")
        } else if line.starts_with("warning:") {
            Some("1;33")
        } else {
            None
        }
    })
}

pub fn sync_success_json(report: &SyncReport) -> Value {
    json!({
        "command": "sync",
        "status": "succeeded",
        "result": report,
    })
}

pub fn sync_failure_json(
    stage: &str,
    message: &str,
    completed_stages: &[String],
    partially_synchronized: bool,
    captured_stderr: &str,
) -> Value {
    let (warnings, recommendations, tool_stderr) = categorize_stderr(captured_stderr);
    json!({
        "command": "sync",
        "status": "failed",
        "error": {
            "stage": stage,
            "message": message,
            "completed_stages": completed_stages,
            "partially_synchronized": partially_synchronized,
            "recovery_guidance": if partially_synchronized {
                Some("Fix the error and rerun `bloomery sync`.")
            } else {
                None
            },
        },
        "notices": {
            "warnings": warnings,
            "recommendations": recommendations,
            "tool_stderr": tool_stderr,
        },
    })
}

pub fn usage_error_json(command: Option<&str>, message: &str) -> Value {
    json!({
        "command": command,
        "status": "failed",
        "error": {
            "kind": "usage",
            "message": message,
        },
    })
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

fn is_failure_id_line(line: &str) -> bool {
    let Some((identifier, _)) = line.split_once(' ') else {
        return false;
    };
    identifier.strip_prefix('f').is_some_and(|digits| {
        !digits.is_empty() && digits.chars().all(|digit| digit.is_ascii_digit())
    })
}

fn colorize_lines(
    text: &str,
    enabled: bool,
    style: impl Fn(&str) -> Option<&'static str>,
) -> String {
    if !enabled {
        return text.to_owned();
    }
    let mut output = String::with_capacity(text.len() + 64);
    for line in text.split_inclusive('\n') {
        let (content, newline) = line
            .strip_suffix('\n')
            .map_or((line, ""), |content| (content, "\n"));
        if let Some(code) = style(content) {
            output.push_str("\x1b[");
            output.push_str(code);
            output.push('m');
            output.push_str(content);
            output.push_str("\x1b[0m");
        } else {
            output.push_str(content);
        }
        output.push_str(newline);
    }
    output
}

#[derive(Debug, Clone, Copy)]
enum Channel {
    Stdout,
    Stderr,
}

/// Line-buffering adapter that adds ANSI styles to the sync command's progress
/// and advisory lines without changing subprocess output.
pub struct ColorWriter<W> {
    inner: W,
    enabled: bool,
    channel: Channel,
    line: Vec<u8>,
}

impl<W> ColorWriter<W> {
    pub fn stdout(inner: W, enabled: bool) -> Self {
        Self {
            inner,
            enabled,
            channel: Channel::Stdout,
            line: Vec::new(),
        }
    }

    pub fn stderr(inner: W, enabled: bool) -> Self {
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
        let style = match self.channel {
            Channel::Stdout if line.starts_with("==>") => Some("1;36"),
            Channel::Stdout if line.starts_with("Successfully synchronized") => Some("1;32"),
            Channel::Stdout if line.starts_with("Skipped ") => Some("1;33"),
            Channel::Stderr if line.starts_with("warning:") => Some("1;33"),
            Channel::Stderr if line.starts_with("recommendation:") => Some("36"),
            _ => None,
        };
        if self.enabled {
            if let Some(style) = style {
                write!(self.inner, "\x1b[{style}m{line}\x1b[0m")?;
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
    use super::{
        ColorWriter, Stream, color_enabled, color_policy, colorize_check, colorize_review,
        diagnostics_json, sync_failure_json, sync_success_json, usage_error_json,
    };
    use bloomery_model::Diagnostic;
    use bloomery_review::ReviewItem;
    use bloomery_sync::SyncReport;
    use bloomery_test_macros::bloomery;
    use std::io::Write;
    use std::path::Path;

    #[test]
    #[bloomery("CLI-INTERFACE-OUTPUT-001")]
    #[bloomery("CLI-INTERFACE-OUTPUT-002")]
    fn color_policy_colors_terminals_and_never_colors_json_or_no_color() {
        assert!(color_policy(true, false, false, false));
        assert!(!color_policy(false, false, false, false));
        assert!(!color_policy(true, true, false, false));
        assert!(!color_policy(true, false, true, false));
        assert!(!color_policy(true, false, false, true));
        assert!(!color_enabled(Stream::Stdout, true));
        assert!(!color_enabled(Stream::Stderr, true));
    }

    #[test]
    #[bloomery("CLI-INTERFACE-OUTPUT-001")]
    #[bloomery("CLI-INTERFACE-OUTPUT-002")]
    fn human_renderers_apply_color_only_when_requested() {
        let check = "PASS: done\nERROR [Missing]:\n  --> spec.toml:2\n";
        let colored_check = colorize_check(check, true);
        assert!(colored_check.contains("\x1b[1;32mPASS:"));
        assert!(colored_check.contains("\x1b[1;31mERROR"));
        assert!(colored_check.contains("\x1b[36m  -->"));
        assert_eq!(colorize_check(check, false), check);

        let review = "PARSER\n└── AUTH\n        └── [REQ-001] Review\n";
        let colored_review = colorize_review(review, true);
        assert!(colored_review.contains("\x1b[1;34mPARSER"));
        assert!(colored_review.contains("\x1b[1;36m└── AUTH"));
        assert!(colored_review.contains("\x1b[1;35m        └── [REQ-001]"));
        assert_eq!(colorize_review(review, false), review);
    }

    #[test]
    #[bloomery("CLI-INTERFACE-OUTPUT-004")]
    #[bloomery("CLI-INTERFACE-OUTPUT-007")]
    fn check_json_uses_repository_relative_structured_diagnostics_without_ansi() {
        let root = Path::new("/repo");
        let diagnostics = [Diagnostic::new("MissingTest", "test missing")
            .at(root.join(".bloomery/specs/REQ.toml"), Some(8))
            .note("add automated coverage")];
        let document = diagnostics_json("check", "failed", root, &diagnostics);
        assert_eq!(document["command"], "check");
        assert_eq!(document["status"], "failed");
        assert_eq!(document["diagnostics"][0]["code"], "MissingTest");
        assert_eq!(
            document["diagnostics"][0]["location"]["path"],
            ".bloomery/specs/REQ.toml"
        );
        assert_eq!(document["diagnostics"][0]["location"]["line"], 8);
        let serialized = serde_json::to_string_pretty(&document).expect("JSON");
        assert!(!serialized.contains("\x1b["));
    }

    #[test]
    #[bloomery("CLI-INTERFACE-OUTPUT-005")]
    fn review_json_retains_the_stable_catalog_fields() {
        let item = ReviewItem {
            area: "CLI".to_owned(),
            feature: "INTERFACE".to_owned(),
            group: "OUTPUT".to_owned(),
            id: "CLI-INTERFACE-OUTPUT-001".to_owned(),
            title: "Colored output".to_owned(),
            statement: "The CLI shall color terminal output.".to_owned(),
            design_ref: "design/output.md".to_owned(),
        };
        let document = serde_json::to_value([item]).expect("review JSON");
        assert!(document.is_array());
        assert_eq!(document[0]["id"], "CLI-INTERFACE-OUTPUT-001");
        assert_eq!(document[0]["design_ref"], "design/output.md");
        assert_eq!(document[0].as_object().expect("review item").len(), 7);
    }

    #[test]
    #[bloomery("CLI-INTERFACE-OUTPUT-006")]
    #[bloomery("CLI-INTERFACE-OUTPUT-007")]
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
        let success_text = serde_json::to_string(&success).expect("JSON");
        assert!(!success_text.contains("\\u001b"));
        assert!(!success_text.contains("==>"));

        let failure = sync_failure_json(
            "final Cargo metadata",
            "exit status 1",
            &["Cargo lock reconciliation".to_owned()],
            true,
            "warning: advisory\nrecommendation: feature\ntool detail\n",
        );
        assert_eq!(failure["error"]["stage"], "final Cargo metadata");
        assert_eq!(failure["error"]["partially_synchronized"], true);
        assert_eq!(
            failure["error"]["recovery_guidance"],
            "Fix the error and rerun `bloomery sync`."
        );
        assert_eq!(failure["notices"]["warnings"][0], "advisory");
        assert_eq!(failure["notices"]["recommendations"][0], "feature");
        assert_eq!(failure["notices"]["tool_stderr"][0], "tool detail");
    }

    #[test]
    #[bloomery("CLI-INTERFACE-OUTPUT-007")]
    fn usage_errors_requested_as_json_are_single_machine_documents() {
        let document = usage_error_json(Some("review"), "unknown option");
        assert_eq!(document["command"], "review");
        assert_eq!(document["status"], "failed");
        assert_eq!(document["error"]["kind"], "usage");
        assert_eq!(document["error"]["message"], "unknown option");
        assert!(
            !serde_json::to_string(&document)
                .expect("JSON")
                .contains("\\u001b")
        );
    }

    #[test]
    #[bloomery("CLI-INTERFACE-OUTPUT-001")]
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
