use bloomery_check_support::check_command::{context::CheckError, model::truncate_utf8};
use bloomery_cli_output as terminal_output;
use std::io::Write;
use std::process::ExitCode;

pub fn write_page(
    json_mode: bool,
    color: terminal_output::ColorMode,
    bytes: Vec<u8>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let bytes = if json_mode {
        bytes
    } else {
        terminal_output::colorize_check(
            &String::from_utf8_lossy(&bytes),
            terminal_output::color_enabled(terminal_output::Stream::Stdout, false, color),
        )
        .into_bytes()
    };
    match stdout.write_all(&bytes).and_then(|()| stdout.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(stderr, "bloomery: unable to write check output: {error}");
            ExitCode::from(2)
        }
    }
}

pub fn report_error(
    json_mode: bool,
    error: &CheckError,
    run: Option<&str>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    if json_mode {
        let (message, _) = truncate_utf8(&error.message, 2048);
        let document = super::output::error_document(error.code, &message, run);
        match super::output::compact_json(&document) {
            Ok(bytes) => {
                let _ = stdout.write_all(&bytes).and_then(|()| stdout.flush());
            }
            Err(message) => {
                let _ = writeln!(stderr, "bloomery: {message}");
            }
        }
    } else {
        let message = format!("error: {}", error.message);
        let rendered = terminal_output::colorize_error(
            &message,
            terminal_output::color_enabled(
                terminal_output::Stream::Stderr,
                false,
                terminal_output::ColorMode::Auto,
            ),
        );
        let _ = writeln!(stderr, "{rendered}");
    }
    ExitCode::from(2)
}
