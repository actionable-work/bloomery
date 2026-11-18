use crate::emit_json;
use bloomery_cli_output::{self as output, Status, error_envelope, success_envelope};
use bloomery_cli_types::SpecArgs;
use bloomery_spec::{SpecError, SpecOutcome, SpecReport};
use serde_json::Value;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

pub(crate) fn run_spec_at(
    args: SpecArgs,
    json_mode: bool,
    color: output::ColorMode,
    root: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    match bloomery_spec::run_at(&args.operation, root) {
        Ok(report) => {
            if json_mode {
                if let Err(code) =
                    emit_json(stdout, stderr, &spec_success_json(&report), "spec result")
                {
                    return code;
                }
            } else {
                render_spec_success(&report, color, stdout);
            }
            ExitCode::SUCCESS
        }
        Err(error) => report_spec_error(error, json_mode, stdout, stderr),
    }
}

fn render_spec_success(report: &SpecReport, color: output::ColorMode, stdout: &mut impl Write) {
    let styled = output::color_enabled(output::Stream::Stdout, false, color);
    let _ = writeln!(stdout, "{}", output::heading("bloomery spec", styled));
    match &report.outcome {
        SpecOutcome::List { records } => {
            for record in records {
                let _ = writeln!(
                    stdout,
                    "[{}] {} ({}/{}/{}, {})",
                    record.id,
                    record.title,
                    record.area,
                    record.feature,
                    record.group,
                    record.design_ref
                );
            }
        }
        SpecOutcome::Show { record } => {
            let _ = writeln!(stdout, "[{}] {}", record.id, record.title);
            let _ = writeln!(
                stdout,
                "{} {} {}",
                record.area, record.feature, record.group
            );
            let mode = if record.manual { "manual" } else { "automated" };
            let _ = writeln!(stdout, "{}", mode);
            let _ = writeln!(stdout, "{}", record.statement);
            let _ = writeln!(stdout, "Design: {}", record.design_ref);
        }
        SpecOutcome::Add {
            id,
            path,
            created_group,
        } => {
            if *created_group {
                let _ = writeln!(stdout, "added {id} and created group {path}");
            } else {
                let _ = writeln!(stdout, "added {id} to {path}");
            }
        }
        SpecOutcome::Set { id, field, changed } => {
            if *changed {
                let _ = writeln!(stdout, "set {id}.{field}");
            } else {
                let _ = writeln!(stdout, "{id}.{field} unchanged");
            }
        }
        SpecOutcome::Remove { id, pruned_group } => {
            if *pruned_group {
                let _ = writeln!(stdout, "removed {id} and pruned its empty group");
            } else {
                let _ = writeln!(stdout, "removed {id}");
            }
        }
        SpecOutcome::Trace { requirements } => {
            for requirement in requirements {
                let _ = writeln!(stdout, "[{}] {}", requirement.id, requirement.title);
                if requirement.references.is_empty() {
                    let _ = writeln!(stdout, "    (no tied tests)");
                }
                for reference in &requirement.references {
                    match reference.line {
                        Some(line) => {
                            let _ = writeln!(
                                stdout,
                                "    {} {}:{line}",
                                reference.scanner, reference.path
                            );
                        }
                        None => {
                            let _ =
                                writeln!(stdout, "    {} {}", reference.scanner, reference.path);
                        }
                    }
                }
            }
        }
        SpecOutcome::TraceReport { report } => render_spec_trace_report(report, stdout),
        SpecOutcome::Candidates { candidates } => {
            if candidates.is_empty() {
                let _ = writeln!(stdout, "no untied tests");
            }
            for candidate in candidates {
                let name = candidate.name.as_deref().unwrap_or("<unnamed>");
                match candidate.line {
                    Some(line) => {
                        let _ = writeln!(
                            stdout,
                            "{} {name} {}:{line}",
                            candidate.scanner, candidate.path
                        );
                    }
                    None => {
                        let _ = writeln!(stdout, "{} {name} {}", candidate.scanner, candidate.path);
                    }
                }
            }
        }
    }
}

fn render_spec_trace_report(report: &bloomery_spec::StaticReport, stdout: &mut impl Write) {
    let _ = writeln!(
        stdout,
        "static report v{} ({})",
        report.version, report.scope.kind
    );
    let _ = writeln!(
        stdout,
        "tree: {:?}; specifications: {:?}; discovery: {:?}; relations: {:?}",
        report.completeness.tree,
        report.completeness.specifications,
        report.completeness.discovery,
        report.completeness.relations
    );
    if let Some(quality) = &report.quality {
        let overrides = quality
            .tests
            .iter()
            .filter(|test| test.exception_reason.is_some())
            .count();
        let _ = writeln!(
            stdout,
            "test breadth: {} tied sites; {overrides} limit exceptions",
            quality.tests.len()
        );
        for test in quality
            .tests
            .iter()
            .filter(|test| test.exception_reason.is_some())
        {
            let _ = writeln!(
                stdout,
                "    {} {}:{} {}: {} requirements; limit {}; exception: {}",
                test.site.scanner,
                test.site.path,
                test.site.line.unwrap_or(1),
                test.name.as_deref().unwrap_or("<unnamed>"),
                test.requirement_ids.len(),
                test.limit,
                test.exception_reason.as_deref().unwrap_or_default()
            );
        }
        let active = quality
            .untied
            .iter()
            .filter(|site| site.exception_reason.is_none())
            .count();
        let _ = writeln!(
            stdout,
            "untied tests: {active} nonexempt, {} exempt; mode {:?}; limit {}",
            quality.untied.len() - active,
            quality.untied_mode,
            quality.max_untied_tests
        );
        for untied in &quality.untied {
            let _ = writeln!(
                stdout,
                "    {} {}:{} {}{}",
                untied.site.scanner,
                untied.site.path,
                untied.site.line.unwrap_or(1),
                untied.name.as_deref().unwrap_or("<unnamed>"),
                untied
                    .exception_reason
                    .as_ref()
                    .map(|reason| format!(" (exception: {reason})"))
                    .unwrap_or_default()
            );
        }
        for package in &quality.crates {
            let _ = writeln!(
                stdout,
                "crate {}: {} requirements; {} feature-linked sites; {}",
                package.manifest,
                package.requirements,
                package.linked_tests,
                if package.excluded {
                    "excluded"
                } else if package.mapped {
                    "mapped"
                } else {
                    "unowned"
                }
            );
            if let Some(reason) = &package.reason {
                let _ = writeln!(stdout, "    {reason}");
            }
        }
    }
    for finding in &report.diagnostics {
        let _ = writeln!(stdout, "[{}] {}", finding.code, finding.message);
        if let Some(location) = &finding.location {
            let _ = writeln!(
                stdout,
                "    {}:{}",
                location.path,
                location.line.unwrap_or(1)
            );
        }
    }
}

fn report_spec_error(
    error: SpecError,
    json_mode: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let exit_code = error.exit_code();
    if json_mode {
        let document = spec_failure_json("spec", error.message());
        if let Err(code) = emit_json(stdout, stderr, &document, "spec error") {
            return code;
        }
    } else {
        let _ = output::write_error(stderr, error.message(), output::ColorMode::Auto);
    }
    ExitCode::from(exit_code)
}

fn spec_success_json(report: &SpecReport) -> Value {
    let result = serde_json::to_value(&report.outcome).unwrap_or(Value::Null);
    success_envelope("spec", None, Status::Succeeded, result, Vec::new())
}

fn spec_failure_json(command: &str, message: &str) -> Value {
    error_envelope(
        Some(command),
        None,
        Status::Error,
        "SpecError",
        "spec",
        message,
        None,
    )
}
