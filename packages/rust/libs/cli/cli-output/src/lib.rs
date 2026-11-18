mod terminal;

pub use terminal::{TerminalFacts, TransientLine};

use bloomery_cli_types::{MISSING_FLAKE_CODE, MISSING_FLAKE_MESSAGE};
use bloomery_model::Diagnostic;
use serde_json::{Map, Value, json};
use std::io::{self, IsTerminal};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

/// Explicit color selection shared by every human-readable output stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    #[default]
    Auto,
    Always,
    Never,
}

impl ColorMode {
    /// Resolve whether a stream is styled. JSON output never carries ANSI, even
    /// when color is forced.
    pub fn resolve(
        self,
        terminal: bool,
        no_color: bool,
        dumb_terminal: bool,
        json_mode: bool,
    ) -> bool {
        match self {
            ColorMode::Never => false,
            ColorMode::Always => !json_mode,
            ColorMode::Auto => terminal && !no_color && !dumb_terminal && !json_mode,
        }
    }
}

pub fn color_enabled(stream: Stream, json_mode: bool, mode: ColorMode) -> bool {
    let terminal = match stream {
        Stream::Stdout => io::stdout().is_terminal(),
        Stream::Stderr => io::stderr().is_terminal(),
    };
    mode.resolve(
        terminal,
        std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty()),
        std::env::var("TERM").is_ok_and(|term| term == "dumb"),
        json_mode,
    )
}

/// Semantic roles shared by every command's human output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Heading,
    Success,
    Failure,
    Warning,
    Info,
    Accent,
    Identifier,
    Muted,
}

impl Role {
    pub fn sgr(self) -> &'static str {
        match self {
            Role::Heading => "1",
            Role::Success => "1;32",
            Role::Failure => "1;31",
            Role::Warning => "1;33",
            Role::Info => "36",
            Role::Accent => "1;36",
            Role::Identifier => "1;35",
            Role::Muted => "2",
        }
    }
}

/// Apply a semantic role to a text fragment. Disabled styling returns the
/// input unchanged so plain output keeps identical content and ordering.
pub fn style(role: Role, text: &str, enabled: bool) -> String {
    if !enabled || text.is_empty() {
        return text.to_owned();
    }
    format!("\x1b[{}m{text}\x1b[0m", role.sgr())
}

/// Render a root heading.
pub fn heading(text: &str, enabled: bool) -> String {
    style(Role::Heading, text, enabled)
}

/// Render a `Label: value` line.
pub fn field(label: &str, value: &str, enabled: bool) -> String {
    format!("{}: {value}", style(Role::Muted, label, enabled))
}

/// Join independent sections with exactly one blank line between them and no
/// trailing blank line.
pub fn join_sections(sections: &[String]) -> String {
    let mut output = String::new();
    for section in sections.iter().filter(|section| !section.is_empty()) {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(section);
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
    output
}

/// Shared status vocabulary for flat outcome lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeStatus {
    Passed,
    Failed,
    Warning,
    Note,
    Skipped,
}

impl OutcomeStatus {
    pub fn glyph(self) -> &'static str {
        match self {
            OutcomeStatus::Passed => "✓",
            OutcomeStatus::Failed => "✗",
            OutcomeStatus::Warning => "!",
            OutcomeStatus::Note => "ℹ",
            OutcomeStatus::Skipped => "·",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            OutcomeStatus::Passed => "passed",
            OutcomeStatus::Failed => "failed",
            OutcomeStatus::Warning => "warning",
            OutcomeStatus::Note => "note",
            OutcomeStatus::Skipped => "skipped",
        }
    }

    pub fn role(self) -> Role {
        match self {
            OutcomeStatus::Passed => Role::Success,
            OutcomeStatus::Failed => Role::Failure,
            OutcomeStatus::Warning => Role::Warning,
            OutcomeStatus::Note => Role::Info,
            OutcomeStatus::Skipped => Role::Muted,
        }
    }

    /// Render `<glyph> <label>` followed by an optional detail.
    pub fn render(self, detail: &str, enabled: bool) -> String {
        let prefix = style(
            self.role(),
            &format!("{} {}", self.glyph(), self.label()),
            enabled,
        );
        if detail.is_empty() {
            prefix
        } else {
            format!("{prefix} · {detail}")
        }
    }
}

/// Command outcome carried by the JSON envelope's `status` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Succeeded,
    Failed,
    Error,
    Interrupted,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Succeeded => "succeeded",
            Status::Failed => "failed",
            Status::Error => "error",
            Status::Interrupted => "interrupted",
        }
    }
}

/// One structured warning in the JSON envelope's `warnings` array.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub code: String,
    pub message: String,
}

impl Warning {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

fn warning_json(warning: &Warning) -> Value {
    json!({
        "code": warning.code,
        "message": warning.message,
    })
}

pub fn attach_warnings(mut value: Value, warnings: &[Warning]) -> Value {
    if warnings.is_empty() {
        return value;
    }
    if let Some(map) = value.as_object_mut() {
        map.insert(
            "warnings".to_owned(),
            Value::Array(warnings.iter().map(warning_json).collect()),
        );
    }
    value
}

fn take_operation(result: &mut Value) -> Option<String> {
    result.as_object_mut().and_then(|map| {
        map.remove("operation")
            .and_then(|value| value.as_str().map(str::to_owned))
    })
}

/// Build a shared success envelope. A command-specific `operation` field is
/// lifted out of the result payload when present.
pub fn success_envelope(
    command: &str,
    operation: Option<&str>,
    status: Status,
    mut result: Value,
    warnings: Vec<Warning>,
) -> Value {
    let operation = operation
        .map(str::to_owned)
        .or_else(|| take_operation(&mut result));
    let mut envelope = Map::new();
    envelope.insert("command".to_owned(), json!(command));
    if let Some(operation) = operation {
        envelope.insert("operation".to_owned(), json!(operation));
    }
    envelope.insert("status".to_owned(), json!(status.as_str()));
    envelope.insert("result".to_owned(), result);
    attach_warnings(Value::Object(envelope), &warnings)
}

/// Build a shared failure envelope.
pub fn error_envelope(
    command: Option<&str>,
    operation: Option<&str>,
    status: Status,
    code: &str,
    kind: &str,
    message: &str,
    details: Option<Value>,
) -> Value {
    let mut error = Map::new();
    error.insert("code".to_owned(), json!(code));
    error.insert("kind".to_owned(), json!(kind));
    error.insert("message".to_owned(), json!(message));
    if let Some(details) = details {
        error.insert("details".to_owned(), details);
    }
    let mut envelope = Map::new();
    envelope.insert("command".to_owned(), json!(command));
    if let Some(operation) = operation {
        envelope.insert("operation".to_owned(), json!(operation));
    }
    envelope.insert("status".to_owned(), json!(status.as_str()));
    envelope.insert("error".to_owned(), Value::Object(error));
    Value::Object(envelope)
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
    let status = match status {
        "failed" => Status::Failed,
        _ => Status::Error,
    };
    error_envelope(
        Some(command),
        None,
        status,
        "Diagnostics",
        "diagnostics",
        "the workspace has validation diagnostics",
        Some(json!({ "diagnostics": values })),
    )
}

fn diagnostic_json(root: &Path, diagnostic: &Diagnostic) -> Value {
    let location = diagnostic.location.as_ref().map(|location| {
        json!({
            "path": location.display_slash_path(root),
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

pub fn colorize_check(text: &str, enabled: bool) -> String {
    colorize_roles(text, enabled, check_line_role)
}

/// Write `error: <message>` as one human-mode stderr line, colored per `mode`.
pub fn write_error(
    stderr: &mut impl io::Write,
    message: impl std::fmt::Display,
    mode: ColorMode,
) -> io::Result<()> {
    let line = format!("error: {message}");
    writeln!(
        stderr,
        "{}",
        colorize_error(&line, color_enabled(Stream::Stderr, false, mode))
    )
}

pub fn colorize_error(text: &str, enabled: bool) -> String {
    colorize_roles(text, enabled, error_line_role)
}

fn check_line_role(line: &str) -> Option<Role> {
    if line.starts_with('✓') || line.starts_with("PASS") {
        Some(Role::Success)
    } else if line.starts_with('✗')
        || line.starts_with("ERROR [")
        || line.starts_with("ERROR  ")
        || line.starts_with("FAIL")
        || line.starts_with("INTERRUPTED")
    {
        Some(Role::Failure)
    } else if line.starts_with("  -->") {
        Some(Role::Info)
    } else if line.starts_with("  = note:") {
        Some(Role::Warning)
    } else if is_failure_id_line(line) {
        Some(Role::Failure)
    } else {
        None
    }
}

fn error_line_role(line: &str) -> Option<Role> {
    if line.starts_with("error:") || line.starts_with("sync failed") {
        Some(Role::Failure)
    } else if line.starts_with("warning:") {
        Some(Role::Warning)
    } else {
        None
    }
}

pub fn usage_error_json(command: Option<&str>, message: &str) -> Value {
    error_envelope(
        command,
        None,
        Status::Error,
        "UsageError",
        "usage",
        message,
        None,
    )
}

/// One structured setup error for a missing root flake, reused by every
/// command so human and JSON modes carry the same guidance.
pub fn flake_setup_error_json(command: Option<&str>) -> Value {
    error_envelope(
        command,
        None,
        Status::Error,
        MISSING_FLAKE_CODE,
        "setup",
        MISSING_FLAKE_MESSAGE,
        None,
    )
}

fn is_failure_id_line(line: &str) -> bool {
    let Some((identifier, _)) = line.split_once(' ') else {
        return false;
    };
    identifier.strip_prefix('f').is_some_and(|digits| {
        !digits.is_empty() && digits.chars().all(|digit| digit.is_ascii_digit())
    })
}

/// Style each line with the role `role` assigns it, when color is enabled.
pub fn colorize_roles(text: &str, enabled: bool, role: impl Fn(&str) -> Option<Role>) -> String {
    if !enabled {
        return text.to_owned();
    }
    let mut output = String::with_capacity(text.len() + 64);
    for line in text.split_inclusive('\n') {
        let (content, newline) = line
            .strip_suffix('\n')
            .map_or((line, ""), |content| (content, "\n"));
        if let Some(role) = role(content) {
            output.push_str(&style(role, content, true));
        } else {
            output.push_str(content);
        }
        output.push_str(newline);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{
        ColorMode, OutcomeStatus, Role, Status, Stream, Warning, color_enabled, colorize_check,
        diagnostics_json, error_envelope, field, heading, join_sections, style, success_envelope,
        usage_error_json,
    };
    use bloomery_model::Diagnostic;
    use std::path::Path;

    fn assert_no_ansi(value: &serde_json::Value) {
        let serialized = serde_json::to_string(value).expect("JSON");
        assert!(!serialized.contains("\\u001b"), "ANSI leaked into JSON");
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-001"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-002"))]
    fn color_policy_colors_terminals_and_never_colors_json_or_no_color() {
        assert!(ColorMode::Auto.resolve(true, false, false, false));
        assert!(!ColorMode::Auto.resolve(false, false, false, false));
        assert!(!ColorMode::Auto.resolve(true, true, false, false));
        assert!(!ColorMode::Auto.resolve(true, false, true, false));
        assert!(!ColorMode::Auto.resolve(true, false, false, true));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-029"))]
    fn forced_color_overrides_redirected_and_no_color_streams() {
        assert!(ColorMode::Always.resolve(false, true, true, false));
        assert!(!ColorMode::Always.resolve(true, false, false, true));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-030"))]
    fn disabled_color_omits_styling_everywhere() {
        assert!(!ColorMode::Never.resolve(true, false, false, false));
        assert!(!ColorMode::Never.resolve(false, false, false, false));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-031"))]
    fn color_modes_cover_the_closed_set() {
        assert_eq!(
            [ColorMode::Auto, ColorMode::Always, ColorMode::Never],
            [ColorMode::default(), ColorMode::Always, ColorMode::Never]
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-PRESENTATION-005"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-PRESENTATION-009"))]
    fn role_palette_is_stable_and_strips_to_plain_text() {
        for role in [
            Role::Heading,
            Role::Success,
            Role::Failure,
            Role::Warning,
            Role::Info,
            Role::Accent,
            Role::Identifier,
            Role::Muted,
        ] {
            let styled = style(role, "value", true);
            assert!(styled.contains("\x1b["));
            assert!(styled.ends_with("\x1b[0m"));
            assert_eq!(style(role, "value", false), "value");
        }
        // The same role always maps to the same SGR regardless of caller.
        assert_eq!(style(Role::Failure, "x", true), "\x1b[1;31mx\x1b[0m");
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-PRESENTATION-004"))]
    fn status_vocabulary_matches_the_presentation_grammar() {
        assert_eq!(OutcomeStatus::Passed.glyph(), "✓");
        assert_eq!(OutcomeStatus::Passed.label(), "passed");
        assert_eq!(OutcomeStatus::Failed.glyph(), "✗");
        assert_eq!(OutcomeStatus::Failed.label(), "failed");
        assert_eq!(OutcomeStatus::Warning.glyph(), "!");
        assert_eq!(OutcomeStatus::Skipped.label(), "skipped");
        assert_eq!(
            OutcomeStatus::Passed.render("28 passed", false),
            "✓ passed · 28 passed"
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-PRESENTATION-006"))]
    fn labeled_values_use_one_format() {
        assert_eq!(
            field("Design", "design/output.md", false),
            "Design: design/output.md"
        );
        assert_eq!(heading("bloomery check", false), "bloomery check");
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-PRESENTATION-013"))]
    fn independent_sections_are_separated_by_one_blank_line() {
        let sections = vec!["a\n".to_owned(), String::new(), "b".to_owned()];
        assert_eq!(join_sections(&sections), "a\n\nb\n");
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-016"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-019"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-021"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-022"))]
    fn success_envelope_carries_command_status_result_and_warnings() {
        let document = success_envelope(
            "config",
            None,
            Status::Succeeded,
            serde_json::json!({"operation": "get", "key": "checks.enable"}),
            vec![Warning::new("missing_flake", "no flake")],
        );
        assert_eq!(document["command"], "config");
        assert_eq!(document["operation"], "get");
        assert_eq!(document["status"], "succeeded");
        assert_eq!(document["result"]["key"], "checks.enable");
        assert!(document["result"].get("operation").is_none());
        assert_eq!(document["warnings"][0]["code"], "missing_flake");
        assert_no_ansi(&document);

        let first = serde_json::to_string(&document).expect("JSON");
        let second = serde_json::to_string(&success_envelope(
            "config",
            None,
            Status::Succeeded,
            serde_json::json!({"operation": "get", "key": "checks.enable"}),
            vec![Warning::new("missing_flake", "no flake")],
        ))
        .expect("JSON");
        assert_eq!(first, second);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-017"))]
    fn error_envelope_carries_a_structured_error() {
        let document = error_envelope(
            Some("build"),
            None,
            Status::Error,
            "MissingNix",
            "operational",
            "nix was not found on PATH",
            None,
        );
        assert_eq!(document["command"], "build");
        assert_eq!(document["status"], "error");
        assert_eq!(document["error"]["code"], "MissingNix");
        assert_eq!(document["error"]["kind"], "operational");
        assert_eq!(document["error"]["message"], "nix was not found on PATH");
        assert_no_ansi(&document);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-018"))]
    fn warnings_are_structured_records() {
        let document = error_envelope(
            Some("sync"),
            None,
            Status::Error,
            "SyncFailed",
            "sync",
            "failed",
            None,
        );
        let document = super::attach_warnings(document, &[Warning::new("warning", "advisory")]);
        assert_eq!(document["warnings"][0]["code"], "warning");
        assert_eq!(document["warnings"][0]["message"], "advisory");
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-020"))]
    fn json_documents_never_contain_ansi() {
        for document in [
            usage_error_json(Some("review"), "unknown option"),
            diagnostics_json(
                "check",
                "failed",
                Path::new("/repo"),
                &[Diagnostic::new("MissingTest", "test missing")],
            ),
        ] {
            assert_no_ansi(&document);
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-004"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-007"))]
    fn check_json_uses_repository_relative_structured_diagnostics_without_ansi() {
        let root = Path::new("/repo");
        let diagnostics = [Diagnostic::new("MissingTest", "test missing")
            .at(root.join(".bloomery/specs/REQ.toml"), Some(8))
            .note("add automated coverage")];
        let document = diagnostics_json("check", "failed", root, &diagnostics);
        assert_eq!(document["command"], "check");
        assert_eq!(document["status"], "failed");
        assert_eq!(
            document["error"]["details"]["diagnostics"][0]["code"],
            "MissingTest"
        );
        assert_eq!(
            document["error"]["details"]["diagnostics"][0]["location"]["path"],
            ".bloomery/specs/REQ.toml"
        );
        assert_eq!(
            document["error"]["details"]["diagnostics"][0]["location"]["line"],
            8
        );
        assert_no_ansi(&document);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-007"))]
    fn usage_errors_requested_as_json_are_single_machine_documents() {
        let document = usage_error_json(Some("review"), "unknown option");
        assert_eq!(document["command"], "review");
        assert_eq!(document["status"], "error");
        assert_eq!(document["error"]["kind"], "usage");
        assert_eq!(document["error"]["code"], "UsageError");
        assert_eq!(document["error"]["message"], "unknown option");
        assert_no_ansi(&document);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-PRESENTATION-005"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-PRESENTATION-009"))]
    fn human_renderers_apply_color_only_when_requested() {
        let check = "✓ passed · 2 checks\n✗ failed\nf1 BuildFailed\n";
        let colored_check = colorize_check(check, true);
        assert!(colored_check.contains("\x1b[1;32m✓ passed"));
        assert!(colored_check.contains("\x1b[1;31m✗ failed"));
        assert_eq!(colorize_check(check, false), check);
    }

    #[test]
    fn color_enabled_accepts_json_and_mode() {
        assert!(!color_enabled(Stream::Stdout, true, ColorMode::Auto));
        assert!(!color_enabled(Stream::Stderr, true, ColorMode::Always));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-PRESENTATION-001"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-PRESENTATION-002"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-PRESENTATION-007"))]
    fn shared_grammar_renders_headings_fields_and_summaries() {
        let plain = join_sections(&[
            heading("bloomery check", false),
            field("Run", "r42", false),
            OutcomeStatus::Passed.render("28 passed", false),
        ]);
        assert_eq!(
            plain,
            "bloomery check\n\nRun: r42\n\n✓ passed · 28 passed\n"
        );

        let styled = join_sections(&[
            heading("bloomery check", true),
            field("Run", "r42", true),
            OutcomeStatus::Passed.render("28 passed", true),
        ]);
        assert!(styled.contains("\x1b[1mbloomery check\x1b[0m"));
        assert!(styled.contains("\x1b[2mRun\x1b[0m: r42"));
        assert!(styled.contains("\x1b[1;32m✓ passed\x1b[0m"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-PRESENTATION-011"))]
    fn automatic_color_targets_only_eligible_terminals() {
        assert!(ColorMode::Auto.resolve(true, false, false, false));
        assert!(!ColorMode::Auto.resolve(true, false, true, false));
        assert!(!ColorMode::Auto.resolve(false, false, false, false));
        assert!(!ColorMode::Auto.resolve(true, true, false, false));
        assert!(!ColorMode::Auto.resolve(true, false, false, true));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-PRESENTATION-012"))]
    fn non_interactive_and_json_output_is_plain() {
        assert!(!ColorMode::Auto.resolve(false, false, false, false));
        assert!(!ColorMode::Auto.resolve(true, true, false, false));
        assert!(!ColorMode::Auto.resolve(true, false, true, false));
        assert!(!ColorMode::Auto.resolve(true, false, false, true));
        assert!(!ColorMode::Never.resolve(true, false, false, false));
    }
}
