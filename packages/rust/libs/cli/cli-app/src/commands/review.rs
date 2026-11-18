use crate::{emit_json, write_diagnostics, write_json};
use bloomery_cli_output as output;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

pub(crate) fn run_review_at(
    root: &Path,
    json_mode: bool,
    color: output::ColorMode,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let context = match bloomery_review::load(root) {
        Ok(context) => context,
        Err(diagnostics) => {
            return report_review_diagnostics(root, json_mode, color, &diagnostics, stdout, stderr);
        }
    };

    let items = bloomery_review::items(&context);
    if json_mode {
        match serde_json::to_value(&items) {
            Ok(items) => {
                let document = output::success_envelope(
                    "review",
                    None,
                    output::Status::Succeeded,
                    items,
                    Vec::new(),
                );
                match emit_json(stdout, stderr, &document, "review catalog") {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(code) => code,
                }
            }
            Err(error) => {
                let document = output::usage_error_json(Some("review"), &error.to_string());
                let _ = write_json(stdout, &document);
                ExitCode::from(1)
            }
        }
    } else {
        let rendered = bloomery_review::render_text(&items);
        let rendered = colorize_review(
            &rendered,
            output::color_enabled(output::Stream::Stdout, false, color),
        );
        let _ = stdout.write_all(rendered.as_bytes());
        ExitCode::SUCCESS
    }
}

fn report_review_diagnostics(
    root: &Path,
    json_mode: bool,
    color: output::ColorMode,
    diagnostics: &[bloomery_model::Diagnostic],
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    if json_mode {
        let document = output::diagnostics_json("review", "failed", root, diagnostics);
        if let Err(code) = emit_json(stdout, stderr, &document, "diagnostics") {
            return code;
        }
    } else {
        write_diagnostics(stderr, root, diagnostics, color);
    }
    ExitCode::from(1)
}

fn colorize_review(text: &str, enabled: bool) -> String {
    output::colorize_roles(text, enabled, review_line_role)
}

fn review_line_role(line: &str) -> Option<output::Role> {
    if line.starts_with("Total manual requirements") {
        Some(output::Role::Success)
    } else if line.starts_with("            Design:") {
        Some(output::Role::Info)
    } else if line.starts_with("        └── [") {
        Some(output::Role::Identifier)
    } else if line.starts_with("    └──") || line.starts_with("└──") {
        Some(output::Role::Accent)
    } else if !line.trim().is_empty() && !line.starts_with(' ') {
        Some(output::Role::Heading)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::assert_no_ansi;
    use bloomery_cli_output::{Status, success_envelope};
    use bloomery_review::ReviewItem;

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-PRESENTATION-005"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-PRESENTATION-009"))]
    fn review_text_applies_color_only_when_requested() {
        let review = "PARSER\n└── AUTH\n        └── [REQ-001] Review\n";
        let colored_review = super::colorize_review(review, true);
        assert!(colored_review.contains("\x1b[1mPARSER"));
        assert!(colored_review.contains("\x1b[1;36m└── AUTH"));
        assert!(colored_review.contains("\x1b[1;35m        └── [REQ-001]"));
        assert_eq!(super::colorize_review(review, false), review);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-005"))]
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
        let document = success_envelope(
            "review",
            None,
            Status::Succeeded,
            serde_json::to_value([item]).expect("review JSON"),
            Vec::new(),
        );
        assert!(document["result"].is_array());
        assert_eq!(document["result"][0]["id"], "CLI-INTERFACE-OUTPUT-001");
        assert_eq!(document["result"][0]["design_ref"], "design/output.md");
        assert_eq!(
            document["result"][0]
                .as_object()
                .expect("review item")
                .len(),
            7
        );
        assert_no_ansi(&document);
    }
}
