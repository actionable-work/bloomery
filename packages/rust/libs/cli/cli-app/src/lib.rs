mod commands;

use bloomery_build_command as build_command;
use bloomery_check_command as check_command;
use bloomery_cli_output as output;
use bloomery_cli_parser::{ParseError, ParseErrorKind, parse_from};
use bloomery_cli_types::{
    CliCommand, CliInvocation, ColorChoice, MISSING_FLAKE_MESSAGE, flake_present,
};
use bloomery_model::render_diagnostics;
#[cfg(test)]
use commands::config::render_config_outcome;
use commands::config::run_config_at;
use commands::disk::run_disk_at;
#[cfg(test)]
use commands::disk::run_disk_with_backend;
use commands::init::run_init_at;
#[cfg(test)]
use commands::init::run_init_request;
use commands::review::run_review_at;
use commands::spec::run_spec_at;
use commands::sync::run_sync_at;
use serde_json::Value;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

/// Parse and execute the CLI request using the current process environment.
pub fn run() -> ExitCode {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing_subscriber::filter::LevelFilter::WARN)
        .with_target(false)
        .without_time()
        .with_writer(io::stderr)
        .try_init();
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    let root = match std::env::current_dir() {
        Ok(root) => root,
        Err(error) => {
            return report_runtime_error(
                "bloomery",
                &error.to_string(),
                false,
                &mut stdout,
                &mut stderr,
            );
        }
    };
    run_args_at(std::env::args_os(), &root, &mut stdout, &mut stderr)
}

/// Root-injected entry point shared by the process entry point and tests.
fn run_args_at<I, T>(
    arguments: I,
    root: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    match parse_from(arguments) {
        Ok(invocation) => run_invocation_at(invocation, root, stdout, stderr),
        Err(error) => report_parse_error_at(error, stdout, stderr),
    }
}

fn run_invocation_at(
    invocation: CliInvocation,
    root: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let color = color_mode(invocation.color);
    if invocation.no_write
        && let Some(operation) = write_required_operation(&invocation.command)
    {
        let command = command_name(&invocation.command);
        let message =
            format!("--no-write cannot be used with {operation} because it writes workspace files");
        return report_usage_error(
            command,
            &message,
            invocation.json_mode,
            color,
            stdout,
            stderr,
        );
    }
    match invocation.command {
        CliCommand::Init(args) => {
            run_init_at(args, invocation.json_mode, color, root, stdout, stderr)
        }
        CliCommand::Sync { update } => run_sync_at(
            root,
            update.as_deref(),
            invocation.json_mode,
            color,
            stdout,
            stderr,
        ),
        CliCommand::Config(args) => {
            run_config_at(args, invocation.json_mode, color, root, stdout, stderr)
        }
        command => run_workspace_command_at_with_mode(
            command,
            invocation.json_mode,
            color,
            invocation.no_write,
            root,
            stdout,
            stderr,
        ),
    }
}

fn write_required_operation(command: &CliCommand) -> Option<String> {
    use bloomery_cli_types::{CheckOperation, ConfigOperation, SpecOperation};
    let writes = match command {
        CliCommand::Init(_) | CliCommand::Sync { .. } => Some(command.name()),
        CliCommand::Config(args) => match &args.operation {
            ConfigOperation::Set { .. }
            | ConfigOperation::Unset { .. }
            | ConfigOperation::Document
            | ConfigOperation::Upgrade {
                dry_run: false,
                diff: false,
            } => Some(args.operation.path()),
            ConfigOperation::Get { .. }
            | ConfigOperation::List { .. }
            | ConfigOperation::Upgrade { .. } => None,
        },
        CliCommand::Spec(args) => match &args.operation {
            SpecOperation::Add { .. }
            | SpecOperation::Set { .. }
            | SpecOperation::Remove { .. } => Some(args.operation.path()),
            SpecOperation::List { .. }
            | SpecOperation::Show { .. }
            | SpecOperation::Trace { .. }
            | SpecOperation::TraceReport { .. }
            | SpecOperation::Candidates => None,
        },
        CliCommand::Check(args) => match &args.operation {
            Some(operation @ CheckOperation::HealthUpdate(_)) => Some(operation.path()),
            _ => None,
        },
        CliCommand::Build(_) | CliCommand::Disk(_) | CliCommand::Review => None,
    };
    writes.map(|path| format!("bloomery {path}"))
}

fn report_usage_error(
    command: Option<&str>,
    message: &str,
    json_mode: bool,
    color: output::ColorMode,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    if json_mode {
        if let Err(code) = emit_json(
            stdout,
            stderr,
            &usage_error_document(command, message),
            "usage error",
        ) {
            return code;
        }
    } else {
        let _ = output::write_error(stderr, message, color);
    }
    ExitCode::from(2)
}

/// JSON usage error in the command's own envelope; `check` uses its check document.
fn usage_error_document(command: Option<&str>, message: &str) -> Value {
    if command == Some("check") {
        check_command::check_output::error_document("UsageError", message, None)
    } else {
        output::usage_error_json(command, message)
    }
}

/// Map the CLI's color selection onto the shared output policy.
fn color_mode(choice: ColorChoice) -> output::ColorMode {
    match choice {
        ColorChoice::Auto => output::ColorMode::Auto,
        ColorChoice::Always => output::ColorMode::Always,
        ColorChoice::Never => output::ColorMode::Never,
    }
}

/// Commands exempt from the required-configuration preflight. `init` is listed
/// ahead of the command existing so it is exempt as soon as it is implemented.
pub(crate) const CONFIGURATION_EXEMPT_COMMANDS: &[&str] = &["help", "init"];

fn configuration_exempt(command: &str) -> bool {
    CONFIGURATION_EXEMPT_COMMANDS.contains(&command)
}

fn command_name(command: &CliCommand) -> Option<&'static str> {
    Some(command.name())
}

fn report_missing_flake(
    command: Option<&str>,
    json_mode: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    if json_mode {
        if let Err(code) = emit_json(
            stdout,
            stderr,
            &output::flake_setup_error_json(command),
            "setup error",
        ) {
            return code;
        }
    } else {
        let _ = output::write_error(stderr, MISSING_FLAKE_MESSAGE, output::ColorMode::Auto);
    }
    ExitCode::from(2)
}

/// Write diagnostics as human-mode stderr text, colored per `mode`.
fn write_diagnostics(
    stderr: &mut impl Write,
    root: &Path,
    diagnostics: &[bloomery_model::Diagnostic],
    mode: output::ColorMode,
) {
    let rendered = render_diagnostics(root, diagnostics);
    let rendered = output::colorize_check(
        &rendered,
        output::color_enabled(output::Stream::Stderr, false, mode),
    );
    let _ = stderr.write_all(rendered.as_bytes());
}

fn report_configuration_error(
    command: Option<&str>,
    root: &Path,
    diagnostic: bloomery_model::Diagnostic,
    json_mode: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    if json_mode {
        let document = output::diagnostics_json(
            command.unwrap_or("bloomery"),
            "failed",
            root,
            std::slice::from_ref(&diagnostic),
        );
        if let Err(code) = emit_json(stdout, stderr, &document, "configuration error") {
            return code;
        }
    } else {
        write_diagnostics(
            stderr,
            root,
            std::slice::from_ref(&diagnostic),
            output::ColorMode::Auto,
        );
    }
    ExitCode::from(1)
}

/// Configuration preflight shared by every repository command that loads a
/// workspace. Parser help and the future `init` command are exempt, so this is
/// only called on the command paths that need configuration.
fn load_configuration_preflight(
    command: Option<&str>,
    root: &Path,
    json_mode: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<(), ExitCode> {
    if let Some(command) = command
        && configuration_exempt(command)
    {
        return Ok(());
    }
    match bloomery_model::config::load(root) {
        Ok(_) => Ok(()),
        Err(diagnostic) => Err(report_configuration_error(
            command, root, diagnostic, json_mode, stdout, stderr,
        )),
    }
}

fn report_parse_error_at(
    error: ParseError,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let exit_code = error.exit_code();
    if matches!(
        error.kind(),
        ParseErrorKind::DisplayHelp | ParseErrorKind::DisplayVersion
    ) {
        let _ = stdout.write_all(error.message().as_bytes());
        return ExitCode::from(exit_code);
    }

    if error.json_mode() {
        let document = usage_error_document(error.command(), error.message());
        if let Err(code) = emit_json(stdout, stderr, &document, "usage error") {
            return code;
        }
    } else {
        let _ = stderr.write_all(error.message().as_bytes());
    }
    ExitCode::from(exit_code)
}

#[cfg(test)]
fn run_workspace_command_at(
    command: CliCommand,
    json_mode: bool,
    color: output::ColorMode,
    root: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    run_workspace_command_at_with_mode(command, json_mode, color, false, root, stdout, stderr)
}

fn run_workspace_command_at_with_mode(
    command: CliCommand,
    json_mode: bool,
    color: output::ColorMode,
    no_write: bool,
    root: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    if !flake_present(root) {
        return report_missing_flake(command_name(&command), json_mode, stdout, stderr);
    }
    if let Err(code) =
        load_configuration_preflight(command_name(&command), root, json_mode, stdout, stderr)
    {
        return code;
    }
    match command {
        CliCommand::Check(args) => {
            check_command::run_at(args, json_mode, color, no_write, root, stdout, stderr)
        }
        CliCommand::Build(args) => {
            build_command::run_at(args, json_mode, color, no_write, root, stdout, stderr)
        }
        CliCommand::Disk(args) => run_disk_at(args, json_mode, color, root, stdout, stderr),
        CliCommand::Review => run_review_at(root, json_mode, color, stdout, stderr),
        CliCommand::Spec(args) => run_spec_at(args, json_mode, color, root, stdout, stderr),
        CliCommand::Init(_) => unreachable!("init is dispatched before workspace loading"),
        CliCommand::Sync { .. } => unreachable!("sync is dispatched before workspace loading"),
        CliCommand::Config(_) => unreachable!("config is dispatched before workspace loading"),
    }
}

#[cfg(test)]
fn run_workspace_command_from_current_dir(
    command: CliCommand,
    json_mode: bool,
    color: output::ColorMode,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let name = command_name(&command).unwrap_or("bloomery");
    let root = match std::env::current_dir() {
        Ok(root) => root,
        Err(error) => {
            return report_runtime_error(name, &error.to_string(), json_mode, stdout, stderr);
        }
    };
    run_workspace_command_at(command, json_mode, color, &root, stdout, stderr)
}

/// Write a JSON document; on failure report `bloomery: unable to render JSON
/// <subject>` on stderr and return the exit code the command should end with.
fn emit_json(
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    value: &Value,
    subject: &str,
) -> Result<(), ExitCode> {
    if write_json(stdout, value).is_err() {
        let _ = writeln!(stderr, "bloomery: unable to render JSON {subject}");
        return Err(ExitCode::from(1));
    }
    Ok(())
}

fn write_json(writer: &mut impl Write, value: &Value) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *writer, value).map_err(io::Error::other)?;
    writer.write_all(b"\n")
}

fn report_runtime_error(
    command: &str,
    message: &str,
    json_mode: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    if json_mode {
        let document = output::error_envelope(
            Some(command),
            None,
            output::Status::Error,
            "RuntimeError",
            "runtime",
            message,
            None,
        );
        if let Err(code) = emit_json(stdout, stderr, &document, "error") {
            return code;
        }
    } else {
        let _ = output::write_error(
            stderr,
            format_args!("bloomery: {message}"),
            output::ColorMode::Auto,
        );
    }
    ExitCode::from(1)
}

#[cfg(test)]
mod tests {
    use super::{
        output, render_config_outcome, run_args_at, run_disk_with_backend,
        run_workspace_command_at, run_workspace_command_from_current_dir,
    };
    use bloomery_cli_parser::{ParseError, ParseErrorKind as ErrorKind, parse_from};
    use bloomery_cli_types::CliCommand as Command;
    use bloomery_cli_types::ColorChoice;
    use bloomery_config::{ConfigOutcome, ConfigReport};
    use serde_json::Value;
    use std::ffi::OsString;
    use std::fs;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::process::ExitCode;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[derive(Debug)]
    struct Cli {
        json: bool,
        no_write: bool,
        command: Command,
    }

    impl Cli {
        fn try_parse_from<I, T>(arguments: I) -> Result<Self, ParseError>
        where
            I: IntoIterator<Item = T>,
            T: Into<OsString> + Clone,
        {
            parse_from(arguments).map(|invocation| Self {
                json: invocation.json_mode,
                no_write: invocation.no_write,
                command: invocation.command,
            })
        }
    }

    static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);
    static CURRENT_DIR_LOCK: Mutex<()> = Mutex::new(());

    fn repository_root() -> PathBuf {
        std::env::var_os("BLOOMERY_REPOSITORY_ROOT")
            .map(PathBuf::from)
            .filter(|root| root.join("Cargo.toml").is_file())
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../.."))
    }

    #[test]
    #[cfg_attr(any(), bloomery("REPOSITORY-LAYOUT-STRUCTURE-007"))]
    fn rust_binary_entrypoints_only_delegate_to_library_crates() {
        let repository_root = repository_root();
        for (path, expected) in [
            (
                "packages/rust/bins/cli/src/main.rs",
                "fn main() -> std::process::ExitCode {\n    bloomery_cli_app::run()\n}\n",
            ),
            (
                "packages/rust/bins/docs/src/main.rs",
                "fn main() {\n    bloomery_docs_server::run();\n}\n",
            ),
        ] {
            let source = fs::read_to_string(repository_root.join(path))
                .unwrap_or_else(|error| panic!("unable to read {path}: {error}"));
            assert_eq!(source, expected, "unexpected binary entry point: {path}");
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("REPOSITORY-LAYOUT-STRUCTURE-008"))]
    fn rust_library_crates_are_grouped_under_the_three_ownership_categories() {
        let repository_root = repository_root();
        let manifest =
            fs::read_to_string(repository_root.join("Cargo.toml")).expect("workspace manifest");
        let members = manifest
            .split("members = [")
            .nth(1)
            .and_then(|members| members.split(']').next())
            .expect("workspace member list");
        let categories = ["shared", "cli", "docs"];
        let mut found_categories = std::collections::BTreeSet::new();

        for member in members
            .lines()
            .map(|line| line.trim().trim_matches(['"', ',']))
        {
            if let Some(path) = member.strip_prefix("packages/rust/libs/") {
                let (category, crate_name) = path.split_once('/').expect("category and crate");
                assert!(
                    categories.contains(&category),
                    "unexpected library category: {category}"
                );
                assert!(
                    !crate_name.contains('/'),
                    "library path has extra levels: {member}"
                );
                found_categories.insert(category);
            }
        }

        assert_eq!(found_categories.len(), categories.len());
    }

    fn parse_update(args: &[&str]) -> Result<Option<String>, ParseError> {
        match Cli::try_parse_from(args)?.command {
            Command::Sync { update } => Ok(update),
            _ => panic!("expected sync command"),
        }
    }

    fn parse_json(args: &[&str]) -> Result<bool, ParseError> {
        Cli::try_parse_from(args).map(|cli| cli.json)
    }

    fn parse_color(args: &[&str]) -> Result<ColorChoice, ParseError> {
        parse_from(args).map(|invocation| invocation.color)
    }

    fn fixture_root(name: &str) -> PathBuf {
        let suffix = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "bloomery-cli-{name}-{}-{suffix}",
            std::process::id()
        ))
    }

    fn config_value_at<'a>(config: &'a toml::Value, path: &[&str]) -> Option<&'a toml::Value> {
        let mut value = config;
        for segment in path {
            value = value.get(*segment)?;
        }
        Some(value)
    }

    fn fixture_workspace(requirement_id: &str, manual: bool) -> PathBuf {
        let root = fixture_root("workspace");
        let area = root.join(".bloomery/specs/CLI");
        let feature = area.join("CHECK");
        fs::create_dir_all(feature.join("design")).expect("design directory");
        fs::create_dir_all(feature.join("requirements")).expect("requirements directory");
        fs::write(root.join("flake.nix"), "{ }\n").expect("flake");
        fs::write(
            root.join(".bloomery/config.toml"),
            "version = 2\n[specs]\ndir = \"specs\"\n",
        )
        .expect("config");
        fs::write(
            area.join("README.md"),
            "---\nid: CLI\nname: CLI\ntagline: CLI\ndescription: Area\n---\n# CLI\n",
        )
        .expect("area README");
        fs::write(
            feature.join("README.md"),
            "---\nid: CHECK\nname: Check\ntagline: Check\ndescription: Feature\n---\n# Check\n",
        )
        .expect("feature README");
        fs::write(feature.join("design/pipeline.md"), "# Pipeline\n").expect("design document");
        fs::write(
            feature.join("requirements/CONTRACT.toml"),
            format!(
                "group = \"CONTRACT\"\n\n[[requirements]]\nid = \"{requirement_id}\"\ntitle = \"Fixture requirement\"\nmanual = {manual}\ndesign = \"design/pipeline.md\"\n\n[requirements.ears]\ntype = \"ubiquitous\"\nsystem = \"the CLI fixture\"\naction = \"exercise the command\"\n"
            ),
        )
        .expect("requirements file");
        root
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-INTERFACE-005"))]
    fn clap_accepts_equals_update_syntax() {
        assert_eq!(
            parse_update(&["bloomery", "sync", "--update=nix,rust"])
                .expect("equals syntax")
                .as_deref(),
            Some("nix,rust")
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-INTERFACE-006"))]
    fn clap_accepts_a_separate_update_argument() {
        assert_eq!(
            parse_update(&["bloomery", "sync", "--update", "nix,rust"])
                .expect("separate argument")
                .as_deref(),
            Some("nix,rust")
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-INTERFACE-008"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-INTERFACE-009"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-INTERFACE-010"))]
    fn clap_distinguishes_bare_update_from_omitted_update() {
        assert_eq!(
            parse_update(&["bloomery", "sync"]).expect("omitted update"),
            None
        );
        assert_eq!(
            parse_update(&["bloomery", "sync", "--update"]).expect("bare update"),
            Some("__bloomery_bare_update__".to_owned())
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-INTERFACE-015"))]
    fn clap_rejects_repeated_update_flags_with_usage_exit_code() {
        let error = parse_update(&["bloomery", "sync", "--update=rust", "--update=nix"])
            .expect_err("duplicate update flags");
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SYNC-INTERFACE-023"))]
    fn cli_does_not_expose_a_lock_subcommand() {
        let error =
            Cli::try_parse_from(["bloomery", "lock"]).expect_err("lock must not be a CLI command");
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-032"))]
    fn no_write_is_global_before_and_after_nested_commands() {
        for arguments in [
            &["bloomery", "--no-write", "check"][..],
            &["bloomery", "check", "--no-write"][..],
            &["bloomery", "--no-write", "config", "get", "checks.enable"][..],
            &["bloomery", "config", "get", "checks.enable", "--no-write"][..],
            &["bloomery", "spec", "list", "--no-write"][..],
        ] {
            let parsed = Cli::try_parse_from(arguments.iter().copied())
                .unwrap_or_else(|error| panic!("arguments {arguments:?}: {error}"));
            assert!(parsed.no_write, "arguments: {arguments:?}");
        }

        let parsed = Cli::try_parse_from(["bloomery", "review"]).expect("default mode");
        assert!(!parsed.no_write);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-001"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-002"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-003"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-014"))]
    fn the_cli_exposes_check_review_and_sync() {
        assert!(matches!(
            Cli::try_parse_from(["bloomery", "check"])
                .expect("check command")
                .command,
            Command::Check(_)
        ));
        assert!(matches!(
            Cli::try_parse_from(["bloomery", "review"])
                .expect("review command")
                .command,
            Command::Review
        ));
        assert!(matches!(
            Cli::try_parse_from(["bloomery", "sync"])
                .expect("sync command")
                .command,
            Command::Sync { update: None }
        ));
        assert!(matches!(
            Cli::try_parse_from(["bloomery", "config", "list"])
                .expect("config command")
                .command,
            Command::Config(_)
        ));
    }

    use bloomery_cli_types::{ConfigOperation, SpecOperation};

    fn parsed_build(arguments: &[&str]) -> bloomery_cli_types::BuildArgs {
        match Cli::try_parse_from(arguments).expect("build parse").command {
            Command::Build(args) => args,
            other => panic!("expected build, got {other:?}"),
        }
    }

    fn parsed_disk(arguments: &[&str]) -> bloomery_cli_types::DiskArgs {
        match Cli::try_parse_from(arguments).expect("disk parse").command {
            Command::Disk(args) => args,
            other => panic!("expected disk, got {other:?}"),
        }
    }

    fn parsed_spec(arguments: &[&str]) -> SpecOperation {
        match Cli::try_parse_from(arguments).expect("spec parse").command {
            Command::Spec(args) => args.operation,
            other => panic!("expected spec, got {other:?}"),
        }
    }

    fn parsed_config(arguments: &[&str]) -> ConfigOperation {
        match Cli::try_parse_from(arguments)
            .expect("config parse")
            .command
        {
            Command::Config(args) => args.operation,
            other => panic!("expected config, got {other:?}"),
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-031"))]
    #[cfg_attr(any(), bloomery("CLI-BUILD-COMMAND-001"))]
    fn build_defaults_select_all_packages_and_the_host() {
        let args = parsed_build(&["bloomery", "build"]);
        assert!(args.packages.is_empty());
        assert!(args.systems.is_empty());
        assert!(!args.check);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-025"))]
    fn build_parses_repeated_package_selectors() {
        assert_eq!(
            parsed_build(&["bloomery", "build", "--package", "app", "--package", "lib*"]).packages,
            ["app", "lib*"]
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-026"))]
    fn build_parses_repeated_system_selectors() {
        assert_eq!(
            parsed_build(&[
                "bloomery",
                "build",
                "--system",
                "x86_64-linux",
                "--system",
                "aarch64-linux"
            ])
            .systems,
            ["x86_64-linux", "aarch64-linux"]
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-027"))]
    fn build_parses_the_optional_check_gate() {
        assert!(parsed_build(&["bloomery", "build", "--check"]).check);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-015"))]
    fn build_accepts_json_selection() {
        assert!(parse_json(&["bloomery", "build", "--json"]).unwrap());
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-028"))]
    fn clap_accepts_color_selection() {
        assert_eq!(
            parse_color(&["bloomery", "review"]).expect("default"),
            ColorChoice::Auto
        );
        assert_eq!(
            parse_color(&["bloomery", "review", "--color", "always"]).expect("separate"),
            ColorChoice::Always
        );
        assert_eq!(
            parse_color(&["bloomery", "--color=never", "check"]).expect("equals"),
            ColorChoice::Never
        );
        assert!(Cli::try_parse_from(["bloomery", "review", "--color", "sometimes"]).is_err());
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-004"))]
    fn clap_provides_its_help_subcommand() {
        for args in [vec!["bloomery", "help"], vec!["bloomery", "help", "check"]] {
            let error = Cli::try_parse_from(args).expect_err("help should be rendered by clap");
            assert_eq!(error.kind(), ErrorKind::DisplayHelp);
            assert_eq!(error.exit_code(), 0);
            assert!(error.to_string().contains("Usage: bloomery"));
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-005"))]
    fn repository_commands_resolve_against_the_process_working_directory() {
        let _lock = CURRENT_DIR_LOCK.lock().expect("working-directory lock");
        let root = fixture_root("cwd");
        fs::create_dir_all(root.join(".bloomery")).expect("config root");
        fs::write(root.join("flake.nix"), "{ }\n").expect("flake");
        fs::write(
            root.join(".bloomery/config.toml"),
            "version = 2\n[specs]\ndir = \"specs\"\n",
        )
        .expect("config");
        let previous = std::env::current_dir().expect("original working directory");
        std::env::set_current_dir(&root).expect("set working directory");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_workspace_command_from_current_dir(
            Command::Review,
            false,
            super::output::ColorMode::Auto,
            &mut stdout,
            &mut stderr,
        );
        std::env::set_current_dir(previous).expect("restore working directory");

        assert_eq!(status, ExitCode::FAILURE);
        assert!(
            String::from_utf8(stderr)
                .expect("diagnostics")
                .contains("SpecManagementDisabled")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-009"))]
    #[cfg_attr(any(), bloomery("CLI-BUILD-COMMAND-002"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-PRESENTATION-008"))]
    fn every_application_command_requires_a_root_flake_before_working() {
        let command_argvs: &[&[&str]] = &[
            &["bloomery", "check"],
            &["bloomery", "check", "list"],
            &["bloomery", "check", "failures"],
            &["bloomery", "check", "details", "f1"],
            &["bloomery", "build"],
            &["bloomery", "disk"],
            &["bloomery", "review"],
            &["bloomery", "sync"],
            &["bloomery", "spec", "list"],
            &["bloomery", "spec", "candidates"],
            &["bloomery", "spec", "trace", "CLI-INTERFACE-COMMANDS-001"],
        ];
        for arguments in command_argvs {
            let root = fixture_root("preflight");
            fs::create_dir_all(&root).expect("workspace root");
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let status = run_args_at(arguments.iter().copied(), &root, &mut stdout, &mut stderr);
            assert_eq!(status, ExitCode::from(2), "arguments: {arguments:?}");
            let text = String::from_utf8_lossy(&stderr);
            assert!(
                text.contains("flake.nix"),
                "arguments: {arguments:?}, stderr: {text:?}"
            );
            assert!(stdout.is_empty(), "arguments: {arguments:?}");
            // The shared preflight stops before any cache, spec, or lock side effect.
            assert!(!root.join(".bloomery").exists());
            assert!(!root.join("Cargo.lock").exists());
            assert!(!root.join("bloomery.lock").exists());
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-008"))]
    fn missing_flake_errors_honor_json_mode() {
        let commands: &[&[&str]] = &[
            &["bloomery", "check", "--json"],
            &["bloomery", "check", "list", "--json"],
            &["bloomery", "build", "--json"],
            &["bloomery", "disk", "--json"],
            &["bloomery", "review", "--json"],
            &["bloomery", "sync", "--json"],
        ];
        for arguments in commands {
            let root = fixture_root("preflight-json");
            fs::create_dir_all(&root).expect("workspace root");
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let status = run_args_at(arguments.iter().copied(), &root, &mut stdout, &mut stderr);
            assert_eq!(status, ExitCode::from(2), "arguments: {arguments:?}");
            assert!(stderr.is_empty(), "arguments: {arguments:?}");
            let document: Value = serde_json::from_slice(&stdout).expect("JSON setup error");
            assert_eq!(document["status"], "error");
            assert_eq!(document["error"]["code"], "MissingFlake");
            assert!(
                document["error"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("flake.nix")
            );
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-028"))]
    fn help_and_version_do_not_require_a_root_flake() {
        let command_argvs: &[&[&str]] = &[
            &["bloomery", "help"],
            &["bloomery", "help", "check"],
            &["bloomery", "--help"],
            &["bloomery", "init", "--help"],
            &["bloomery", "check", "--help"],
        ];
        for arguments in command_argvs {
            let root = fixture_root("help-no-flake");
            fs::create_dir_all(&root).expect("workspace root");
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let status = run_args_at(arguments.iter().copied(), &root, &mut stdout, &mut stderr);
            assert_eq!(status, ExitCode::SUCCESS, "arguments: {arguments:?}");
            assert!(!stdout.is_empty(), "arguments: {arguments:?}");
            assert!(
                stderr.is_empty(),
                "arguments: {arguments:?}, stderr: {}",
                String::from_utf8_lossy(&stderr)
            );
            // Help output must not create any cache, spec, or lock side effect.
            assert!(!root.join(".bloomery").exists());
            assert!(!root.join("Cargo.lock").exists());
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-036"))]
    fn no_write_keeps_parser_help_and_version_available() {
        for arguments in [
            &["bloomery", "--no-write", "--help"][..],
            &["bloomery", "check", "--no-write", "--help"][..],
            &["bloomery", "--no-write", "--version"][..],
        ] {
            let root = fixture_root("no-write-help");
            fs::create_dir_all(&root).expect("workspace root");
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let status = run_args_at(arguments.iter().copied(), &root, &mut stdout, &mut stderr);
            assert_eq!(status, ExitCode::SUCCESS, "arguments: {arguments:?}");
            assert!(!stdout.is_empty());
            assert!(stderr.is_empty());
            assert!(!root.join(".bloomery").exists());
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-033"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-COMMAND-013"))]
    #[cfg_attr(any(), bloomery("CLI-SYNC-INTERFACE-024"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-024"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-EDITING-021"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-027"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-DOCUMENT-013"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-HEALTH-038"))]
    fn no_write_rejects_writers_before_workspace_preflights() {
        let commands: &[&[&str]] = &[
            &["bloomery", "init", "--no-write"],
            &["bloomery", "sync", "--no-write"],
            &[
                "bloomery",
                "spec",
                "add",
                "CLI-EXAMPLE-REQ-001",
                "--title",
                "Example",
                "--ears",
                "{ type = \"ubiquitous\", system = \"test\", action = \"pass\" }",
                "--no-write",
            ],
            &[
                "bloomery",
                "spec",
                "set",
                "CLI-EXAMPLE-REQ-001",
                "title",
                "Example",
                "--no-write",
            ],
            &[
                "bloomery",
                "spec",
                "remove",
                "CLI-EXAMPLE-REQ-001",
                "--no-write",
            ],
            &[
                "bloomery",
                "config",
                "set",
                "checks.enable",
                "true",
                "--no-write",
            ],
            &["bloomery", "config", "unset", "checks.enable", "--no-write"],
            &["bloomery", "config", "upgrade", "--no-write"],
            &["bloomery", "config", "document", "--no-write"],
            &["bloomery", "check", "health", "update", "--no-write"],
        ];
        for (index, arguments) in commands.iter().enumerate() {
            let root = fixture_root(&format!("no-write-reject-{index}"));
            fs::create_dir_all(&root).expect("workspace root");
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let status = run_args_at(*arguments, &root, &mut stdout, &mut stderr);
            assert_eq!(status, ExitCode::from(2), "arguments: {arguments:?}");
            assert!(
                String::from_utf8_lossy(&stderr).contains("--no-write"),
                "arguments: {arguments:?}, stderr: {}",
                String::from_utf8_lossy(&stderr)
            );
            assert!(!root.join("flake.nix").exists());
            assert!(!root.join(".bloomery/config.toml").exists());
            assert!(!root.join("Cargo.lock").exists());
            assert!(!root.join("bloomery.lock").exists());
            assert!(!root.join("Cargo.toml").exists());
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-035"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-UPGRADE-028"))]
    fn no_write_allows_reads_builds_and_read_only_upgrade_modes() {
        for arguments in [
            &["bloomery", "review", "--no-write"][..],
            &["bloomery", "disk", "--no-write"][..],
            &["bloomery", "build", "--no-write"][..],
            &["bloomery", "config", "get", "checks.enable", "--no-write"][..],
            &["bloomery", "config", "list", "--no-write"][..],
            &["bloomery", "config", "upgrade", "--dry-run", "--no-write"][..],
            &["bloomery", "config", "upgrade", "--diff", "--no-write"][..],
            &["bloomery", "spec", "list", "--no-write"][..],
            &["bloomery", "spec", "candidates", "--no-write"][..],
            &["bloomery", "check", "list", "--no-write"][..],
            &["bloomery", "check", "failures", "--no-write"][..],
            &["bloomery", "check", "details", "f1", "--no-write"][..],
            &["bloomery", "check", "health", "--no-write"][..],
        ] {
            let parsed = parse_from(arguments.iter().copied())
                .unwrap_or_else(|error| panic!("arguments {arguments:?}: {error}"));
            assert!(parsed.no_write, "arguments: {arguments:?}");
            assert_eq!(
                super::write_required_operation(&parsed.command),
                None,
                "arguments: {arguments:?}"
            );
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-009"))]
    fn a_present_root_flake_allows_command_dispatch() {
        let root = fixture_root("present-flake");
        fs::create_dir_all(root.join(".bloomery")).expect("workspace root");
        fs::write(root.join("flake.nix"), "{ }\n").expect("flake");
        fs::write(
            root.join(".bloomery/config.toml"),
            "version = 2\n[specs]\ndir = \"specs\"\n",
        )
        .expect("config");

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_args_at(["bloomery", "--help"], &root, &mut stdout, &mut stderr);
        assert_eq!(status, ExitCode::SUCCESS);
        assert!(String::from_utf8_lossy(&stdout).contains("Usage: bloomery"));

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_args_at(["bloomery", "review"], &root, &mut stdout, &mut stderr);
        assert_eq!(status, ExitCode::FAILURE);
        assert!(String::from_utf8_lossy(&stderr).contains("SpecManagementDisabled"));

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_args_at(["bloomery", "sync"], &root, &mut stdout, &mut stderr);
        assert_eq!(status, ExitCode::from(1));
        assert!(String::from_utf8_lossy(&stderr).contains("Cargo.toml"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-010"))]
    #[cfg_attr(any(), bloomery("CLI-BUILD-COMMAND-003"))]
    fn repository_commands_require_configuration() {
        for arguments in [
            &["bloomery", "check"][..],
            &["bloomery", "check", "list"][..],
            &["bloomery", "build"][..],
            &["bloomery", "disk"][..],
            &["bloomery", "review"][..],
            &["bloomery", "sync"][..],
            &["bloomery", "spec", "list"][..],
            &["bloomery", "spec", "candidates"][..],
        ] {
            let root = fixture_root("missing-config");
            fs::create_dir_all(&root).expect("workspace root");
            fs::write(root.join("flake.nix"), "{ }\n").expect("flake");
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let status = run_args_at(arguments.iter().copied(), &root, &mut stdout, &mut stderr);
            assert_eq!(status, ExitCode::from(1), "arguments: {arguments:?}");
            let text = String::from_utf8_lossy(&stderr);
            assert!(
                text.contains("ConfigurationError"),
                "arguments: {arguments:?}, stderr: {text:?}"
            );
            assert!(!root.join("Cargo.lock").exists());
            assert!(!root.join("bloomery.lock").exists());
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-010"))]
    fn missing_configuration_errors_honor_json_mode() {
        let root = fixture_root("missing-config-json");
        fs::create_dir_all(&root).expect("workspace root");
        fs::write(root.join("flake.nix"), "{ }\n").expect("flake");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_args_at(
            ["bloomery", "check", "--json"],
            &root,
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(status, ExitCode::from(1));
        assert!(stderr.is_empty());
        let document: Value = serde_json::from_slice(&stdout).expect("JSON configuration error");
        assert_eq!(document["status"], "failed");
        assert_eq!(
            document["error"]["details"]["diagnostics"][0]["code"],
            "ConfigurationError"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-011"))]
    fn parser_help_does_not_require_configuration() {
        let root = fixture_root("help-missing-config");
        fs::create_dir_all(&root).expect("workspace root");
        fs::write(root.join("flake.nix"), "{ }\n").expect("flake");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_args_at(["bloomery", "--help"], &root, &mut stdout, &mut stderr);
        assert_eq!(status, ExitCode::SUCCESS);
        assert!(String::from_utf8_lossy(&stdout).contains("Usage: bloomery"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-012"))]
    fn init_is_exempt_from_the_configuration_requirement() {
        assert!(super::configuration_exempt("init"));
        assert!(super::configuration_exempt("help"));
        assert!(!super::configuration_exempt("check"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-001"))]
    fn review_defaults_to_human_readable_output() {
        let root = fixture_workspace("CLI-CHECK-CONTRACT-001", true);
        let json_mode = parse_json(&["bloomery", "review"]).expect("default output mode");
        assert!(!json_mode);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_workspace_command_at(
            Command::Review,
            json_mode,
            super::output::ColorMode::Auto,
            &root,
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(status, ExitCode::SUCCESS);
        assert!(stderr.is_empty());
        let text = String::from_utf8(stdout).expect("human-readable output");
        assert!(
            !text.trim_start().starts_with('['),
            "review output should be text"
        );
        assert!(text.contains("CLI"));
        assert!(text.contains("Fixture requirement"));
        assert!(text.contains("Total manual requirements requiring review: 1"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-002"))]
    fn review_emits_json_when_requested() {
        let root = fixture_workspace("CLI-CHECK-CONTRACT-001", true);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_workspace_command_at(
            Command::Review,
            true,
            super::output::ColorMode::Auto,
            &root,
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(status, ExitCode::SUCCESS);
        assert!(stderr.is_empty());
        let document: Value = serde_json::from_slice(&stdout).expect("review JSON envelope");
        let items = document["result"].as_array().expect("review result array");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "CLI-CHECK-CONTRACT-001");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-REVIEW-CATALOG-003"))]
    fn review_requires_spec_management() {
        let root = fixture_root("review-disabled");
        fs::create_dir_all(root.join(".bloomery")).expect("workspace root");
        fs::write(root.join("flake.nix"), "{ }\n").expect("flake");
        fs::write(
            root.join(".bloomery/config.toml"),
            "version = 2\n[specs]\nenable = false\n",
        )
        .expect("configuration");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_workspace_command_at(
            Command::Review,
            true,
            super::output::ColorMode::Auto,
            &root,
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(status, ExitCode::from(1));
        let rendered = String::from_utf8(stdout).expect("review error output");
        assert!(rendered.contains("SpecManagementDisabled"));
        assert!(rendered.contains("disabled"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-003"))]
    fn all_application_commands_accept_the_same_json_flag() {
        let commands: &[&[&str]] = &[
            &["bloomery", "check", "--json"],
            &["bloomery", "init", "--json"],
            &["bloomery", "review", "--json"],
            &["bloomery", "sync", "--json"],
            &["bloomery", "config", "list", "--json"],
        ];
        for command in commands {
            assert!(parse_json(command).expect("--json parses"));
        }
        assert!(parse_json(&["bloomery", "--json", "check"]).expect("global --json"));
        assert!(Cli::try_parse_from(["bloomery", "review", "--format", "json"]).is_err());
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-006"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-007"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-008"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-SELECT-010"))]
    fn check_parses_selection_and_retrieval_commands() {
        for args in [
            &["bloomery", "check", "--check", "static:*", "--jobs", "2"][..],
            &["bloomery", "check", "--system", "x86_64-linux"][..],
            &[
                "bloomery",
                "check",
                "list",
                "--check",
                "static:*",
                "--system",
                "x86_64-linux",
            ][..],
            &[
                "bloomery", "check", "list", "--check", "static:*", "--limit", "2",
            ][..],
            &["bloomery", "check", "failures", "--offset", "1"][..],
            &["bloomery", "check", "details", "f1", "--run", "r1"][..],
        ] {
            assert!(Cli::try_parse_from(args).is_ok(), "{args:?}");
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-007"))]
    fn check_retrieval_parses_global_json() {
        for args in [
            &["bloomery", "--json", "check", "list"][..],
            &["bloomery", "check", "list", "--json"][..],
        ] {
            assert!(parse_json(args).unwrap());
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-008"))]
    fn check_retrieval_rejects_execution_flags() {
        for args in [
            &["bloomery", "check", "list", "--jobs", "2"][..],
            &["bloomery", "check", "failures", "--fail-fast"][..],
            &["bloomery", "check", "details", "f1", "--check", "static:* "][..],
        ] {
            assert!(Cli::try_parse_from(args).is_err(), "{args:?}");
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-009"))]
    fn check_rejects_invalid_numeric_options() {
        for args in [
            &["bloomery", "check", "--jobs", "not-a-number"][..],
            &["bloomery", "check", "list", "--offset", "-1"][..],
        ] {
            assert!(Cli::try_parse_from(args).is_err(), "{args:?}");
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-029"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-030"))]
    #[cfg_attr(any(), bloomery("CLI-DISK-COMMAND-008"))]
    #[cfg_attr(any(), bloomery("CLI-DISK-COMMAND-011"))]
    fn disk_parses_tree_and_addressed_forms() {
        let default = parsed_disk(&["bloomery", "disk"]);
        assert!(!default.tree);
        assert_eq!(default.derivation, None);
        assert!(default.systems.is_empty());
        assert_eq!(default.scope, bloomery_cli_types::DiskScope::All);
        let addressed = parsed_disk(&["bloomery", "disk", "packages.x86_64-linux.default"]);
        assert!(!addressed.tree);
        assert_eq!(
            addressed.derivation.as_deref(),
            Some("packages.x86_64-linux.default")
        );
        assert!(addressed.systems.is_empty());
        let tree = parsed_disk(&["bloomery", "disk", "tree"]);
        assert!(tree.tree);
        assert_eq!(tree.derivation, None);
        assert_eq!(tree.scope, bloomery_cli_types::DiskScope::All);
        let addressed_tree =
            parsed_disk(&["bloomery", "disk", "tree", "packages.x86_64-linux.default"]);
        assert!(addressed_tree.tree);
        assert_eq!(
            addressed_tree.derivation.as_deref(),
            Some("packages.x86_64-linux.default")
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-021"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-022"))]
    fn disk_parses_systems_and_rejects_addressed_system_scope() {
        assert_eq!(
            parsed_disk(&[
                "bloomery",
                "disk",
                "--system",
                "x86_64-linux",
                "--system",
                "aarch64-linux",
                "--system",
                "x86_64-linux"
            ])
            .systems,
            ["x86_64-linux", "aarch64-linux"]
        );
        assert_eq!(
            parsed_disk(&[
                "bloomery",
                "disk",
                "tree",
                "--system",
                "x86_64-linux",
                "--system",
                "aarch64-linux"
            ])
            .systems,
            ["x86_64-linux", "aarch64-linux"]
        );
        for args in [
            &[
                "bloomery",
                "disk",
                "--system",
                "x86_64-linux",
                "packages.x86_64-linux.default",
            ][..],
            &[
                "bloomery",
                "disk",
                "tree",
                "--system",
                "x86_64-linux",
                "packages.x86_64-linux.default",
            ][..],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-023"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-024"))]
    fn disk_parses_measurement_scope_and_rejects_addressed_scope() {
        assert_eq!(
            parsed_disk(&["bloomery", "disk", "--scope", "runtime"]).scope,
            bloomery_cli_types::DiskScope::Runtime
        );
        assert_eq!(
            parsed_disk(&["bloomery", "disk", "tree", "--scope", "build"]).scope,
            bloomery_cli_types::DiskScope::Build
        );
        assert!(
            Cli::try_parse_from([
                "bloomery",
                "disk",
                "--scope",
                "runtime",
                "packages.x86_64-linux.default"
            ])
            .is_err()
        );
        assert!(Cli::try_parse_from(["bloomery", "disk", "--scope", "nonsense"]).is_err());
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-029"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-013"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-014"))]
    fn disk_json_reports_measurement_through_the_cli() {
        struct Fake;

        fn target(category: &str, name: &str, path: &str) -> bloomery_disk::DiskTarget {
            bloomery_disk::DiskTarget {
                category: category.to_owned(),
                name: name.to_owned(),
                path: path.to_owned(),
            }
        }

        impl bloomery_disk::NixQuery for Fake {
            fn is_available(&self) -> bool {
                true
            }

            fn host_system(&self, _root: &Path) -> Result<String, String> {
                Ok("x86_64-linux".to_owned())
            }

            fn resolve_tree(
                &self,
                _root: &Path,
                _systems: &[String],
                _scope: bloomery_disk::DiskScope,
            ) -> Result<Vec<bloomery_disk::DiskTarget>, String> {
                Ok(vec![target("runtime", "default", "/nix/store/aaa-app")])
            }

            fn resolve_derivation(
                &self,
                _root: &Path,
                address: &str,
            ) -> Result<Vec<bloomery_disk::DiskTarget>, String> {
                Ok(vec![target("derivation", address, address)])
            }

            fn closure_sizes(
                &self,
                _root: &Path,
                targets: &[bloomery_disk::DiskTarget],
            ) -> Result<Vec<bloomery_disk::TargetClosure>, String> {
                Ok(targets
                    .iter()
                    .map(|target| bloomery_disk::TargetClosure {
                        target: target.clone(),
                        realized: true,
                        entries: vec![bloomery_disk::PathSize {
                            path: target.path.clone(),
                            nar_size: 2048,
                        }],
                    })
                    .collect())
            }
        }

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_disk_with_backend(
            bloomery_cli_types::DiskArgs::default(),
            true,
            super::output::ColorMode::Auto,
            Path::new("/repo"),
            &Fake,
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(status, ExitCode::SUCCESS);
        assert!(stderr.is_empty());
        let document: Value = serde_json::from_slice(&stdout).expect("disk JSON");
        assert_eq!(document["command"], "disk");
        assert_eq!(document["status"], "succeeded");
        assert_eq!(document["result"]["bytes"], 2048);
        assert_eq!(document["result"]["systems"][0], "x86_64-linux");
        assert_eq!(document["result"]["categories"][0]["name"], "runtime");
        assert_eq!(
            document["result"]["categories"][0]["entries"][0]["name"],
            "default"
        );
        assert_eq!(
            document["result"]["categories"][0]["entries"][0]["bytes"],
            2048
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-005"))]
    fn every_application_command_accepts_short_and_long_help_flags() {
        for command in ["check", "disk", "init", "review", "sync", "config"] {
            for flag in ["-h", "--help"] {
                let error = Cli::try_parse_from(["bloomery", command, flag])
                    .expect_err("help flag should short-circuit parsing");
                assert_eq!(error.kind(), ErrorKind::DisplayHelp);
                assert_eq!(error.exit_code(), 0);
            }
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-003"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-006"))]
    fn every_application_command_accepts_the_shared_json_flag_before_or_after_its_name() {
        for command in ["check", "disk", "init", "review", "sync"] {
            assert!(parse_json(&["bloomery", command, "--json"]).expect("trailing --json"));
            assert!(parse_json(&["bloomery", "--json", command]).expect("leading --json"));
        }
        for args in [
            ["bloomery", "spec", "list", "--json"],
            ["bloomery", "--json", "spec", "list"],
            ["bloomery", "spec", "candidates", "--json"],
            ["bloomery", "--json", "spec", "candidates"],
        ] {
            assert!(parse_json(&args).expect("spec --json"));
        }
        assert!(parse_json(&["bloomery", "config", "list", "--json"]).expect("trailing --json"));
        assert!(parse_json(&["bloomery", "--json", "config", "list"]).expect("leading --json"));
        assert!(Cli::try_parse_from(["bloomery", "review", "--format", "json"]).is_err());
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-020"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-021"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-015"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-016"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-017"))]
    fn spec_list_parses_its_filters() {
        assert_eq!(
            parsed_spec(&["bloomery", "spec", "list"]),
            SpecOperation::List {
                area: None,
                feature: None,
                group: None
            }
        );
        assert_eq!(
            parsed_spec(&[
                "bloomery",
                "spec",
                "list",
                "--area",
                "A",
                "--feature",
                "F",
                "--group",
                "G"
            ]),
            SpecOperation::List {
                area: Some("A".into()),
                feature: Some("F".into()),
                group: Some("G".into())
            }
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-022"))]
    fn spec_show_parses_the_requirement_id() {
        assert_eq!(
            parsed_spec(&["bloomery", "spec", "show", "A-F-G-001"]),
            SpecOperation::Show {
                id: "A-F-G-001".into()
            }
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-023"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-019"))]
    fn spec_add_parses_required_and_optional_fields() {
        let ears = "{ type = \"ubiquitous\", system = \"s\", action = \"a\" }";
        assert_eq!(
            parsed_spec(&[
                "bloomery",
                "spec",
                "add",
                "A-F-G-001",
                "--title",
                "Title",
                "--ears",
                ears,
                "--design",
                "design/x.md",
                "--manual"
            ]),
            SpecOperation::Add {
                id: "A-F-G-001".into(),
                title: "Title".into(),
                ears: ears.into(),
                design: Some("design/x.md".into()),
                manual: true
            }
        );
        assert!(Cli::try_parse_from(["bloomery", "spec", "add", "ID", "--ears", "{}"]).is_err());
        assert!(Cli::try_parse_from(["bloomery", "spec", "add", "ID", "--title", "T"]).is_err());
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-024"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-020"))]
    fn spec_set_parses_the_addressed_field_and_value() {
        assert_eq!(
            parsed_spec(&["bloomery", "spec", "set", "A-F-G-001", "title", "New"]),
            SpecOperation::Set {
                id: "A-F-G-001".into(),
                field: "title".into(),
                value: "New".into()
            }
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-025"))]
    fn spec_remove_parses_the_requirement_id() {
        assert_eq!(
            parsed_spec(&["bloomery", "spec", "remove", "A-F-G-001"]),
            SpecOperation::Remove {
                id: "A-F-G-001".into()
            }
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-026"))]
    fn spec_trace_parses_ids_and_report_scope() {
        assert_eq!(
            parsed_spec(&["bloomery", "spec", "trace", "A-F-G-001", "A-F-G-002"]),
            SpecOperation::Trace {
                ids: vec!["A-F-G-001".into(), "A-F-G-002".into()]
            }
        );
        assert_eq!(
            parsed_spec(&["bloomery", "spec", "trace", "--report"]),
            SpecOperation::TraceReport { rust_cfg: None }
        );
        for arguments in [
            &["bloomery", "spec", "trace"][..],
            &["bloomery", "spec", "trace", "--report", "A-F-G-001"][..],
            &["bloomery", "spec", "trace", "A-F-G-001", "--rust-cfg", "{}"][..],
            &["bloomery", "spec", "trace", "--rust-cfg", "{}"][..],
        ] {
            assert!(Cli::try_parse_from(arguments).is_err());
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-027"))]
    fn spec_candidates_parses_without_arguments() {
        assert_eq!(
            parsed_spec(&["bloomery", "spec", "candidates"]),
            SpecOperation::Candidates
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-018"))]
    fn spec_filter_flags_are_rejected_outside_list() {
        for flag in ["--area", "--feature", "--group"] {
            assert!(Cli::try_parse_from(["bloomery", "spec", "show", "ID", flag, "A"]).is_err());
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-010"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-011"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-012"))]
    fn spec_json_contracts() {
        let root = fixture_workspace("CLI-CHECK-CONTRACT-001", false);

        for (arguments, operation) in [
            (vec!["bloomery", "spec", "list", "--json"], "list"),
            (
                vec![
                    "bloomery",
                    "spec",
                    "show",
                    "CLI-CHECK-CONTRACT-001",
                    "--json",
                ],
                "show",
            ),
            (
                vec![
                    "bloomery",
                    "spec",
                    "trace",
                    "CLI-CHECK-CONTRACT-001",
                    "--json",
                ],
                "trace",
            ),
            (
                vec!["bloomery", "spec", "candidates", "--json"],
                "candidates",
            ),
        ] {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let status = run_args_at(arguments.iter().copied(), &root, &mut stdout, &mut stderr);
            assert_eq!(status, ExitCode::SUCCESS, "arguments: {arguments:?}");
            let document: Value = serde_json::from_slice(&stdout).expect("spec JSON");
            assert_eq!(document["command"], "spec");
            assert_eq!(document["status"], "succeeded");
            assert_eq!(document["operation"], operation);
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-004"))]
    fn top_level_help_is_generated_by_the_parser() {
        let error = Cli::try_parse_from(["bloomery", "--help"]).expect_err("help output");
        assert_eq!(error.kind(), ErrorKind::DisplayHelp);
        assert_eq!(error.exit_code(), 0);
        assert!(error.to_string().contains("Usage: bloomery"));
    }

    #[derive(Default)]
    struct InitLock {
        locked: bool,
    }

    impl bloomery_init::LockStage for InitLock {
        fn missing_tools(&self) -> Vec<&'static str> {
            Vec::new()
        }

        fn lock<O: Write, E: Write>(
            &mut self,
            _root: &Path,
            _stdout: &mut O,
            _stderr: &mut E,
        ) -> Result<Vec<String>, bloomery_init::InitError> {
            self.locked = true;
            Ok(vec![
                "Cargo.lock".to_owned(),
                "bloomery.lock".to_owned(),
                "flake.lock".to_owned(),
            ])
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-013"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-COMMAND-001"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-COMMAND-002"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-COMMAND-003"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-COMMAND-008"))]
    fn init_exposes_target_template_and_force_options() {
        let parsed =
            Cli::try_parse_from(["bloomery", "init", "app", "--template", "axum", "--force"])
                .expect("init parses");
        match parsed.command {
            Command::Init(args) => {
                assert_eq!(args.directory.as_deref(), Some(Path::new("app")));
                assert_eq!(args.template.as_deref(), Some("axum"));
                assert!(args.force);
            }
            other => panic!("expected init, got {other:?}"),
        }
        let default = Cli::try_parse_from(["bloomery", "init"]).expect("init defaults");
        match default.command {
            Command::Init(args) => {
                assert!(args.directory.is_none());
                assert!(args.template.is_none());
                assert!(!args.force);
            }
            other => panic!("expected init, got {other:?}"),
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INIT-FILES-007"))]
    fn init_templates_materialize_all_recommended_config_values() {
        for template in bloomery_init::TEMPLATE_NAMES {
            let root = fixture_root(&format!("init-config-{template}"));
            let target = root.join("app");
            let request = bloomery_init::InitRequest {
                directory: target.clone(),
                template: Some((*template).to_owned()),
                force: false,
            };
            let mut stage = InitLock::default();
            bloomery_init::run(&request, &mut stage, &mut Vec::new(), &mut Vec::new())
                .expect("template scaffolds");
            assert!(stage.locked, "{template} lock stage");

            let config_text =
                fs::read_to_string(target.join(".bloomery/config.toml")).expect("config file");
            let config: toml::Value = toml::from_str(&config_text).expect("valid TOML");
            for entry in bloomery_model::catalog::recommended_entries() {
                let expected = entry
                    .recommended
                    .expect("recommended entry carries a value")
                    .to_toml();
                assert_eq!(
                    config_value_at(&config, entry.path),
                    Some(&expected),
                    "{template}: {}",
                    entry.path.join(".")
                );
            }
            for entry in bloomery_model::catalog::CATALOG
                .iter()
                .filter(|entry| entry.recommended.is_none())
            {
                assert_eq!(
                    config_value_at(&config, entry.path),
                    None,
                    "{template}: {} has no recommended value",
                    entry.path.join(".")
                );
            }

            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INIT-COMMAND-006"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-OUTPUT-002"))]
    #[cfg_attr(any(), bloomery("CLI-INIT-OUTPUT-004"))]
    fn init_runs_without_a_flake_or_configuration() {
        let root = fixture_root("init-no-flake");
        let request = bloomery_init::InitRequest {
            directory: root.join("app"),
            template: None,
            force: false,
        };
        let mut stage = InitLock::default();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = super::run_init_request(
            &request,
            &mut stage,
            false,
            super::output::ColorMode::Auto,
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(status, ExitCode::SUCCESS);
        assert!(stage.locked);
        assert!(root.join("app/flake.nix").is_file());
        assert!(!root.join("flake.nix").exists());
        let text = String::from_utf8(stdout).expect("human output");
        assert!(text.contains("basic"));
        assert!(text.contains("nix develop"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-016"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-017"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-COMMANDS-009"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-COMMANDS-010"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-COMMANDS-011"))]
    fn config_is_exempt_from_the_flake_preflight_and_warns() {
        let root = fixture_root("config-no-flake");
        fs::create_dir_all(&root).expect("workspace root");

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_args_at(
            ["bloomery", "config", "list"],
            &root,
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(status, ExitCode::SUCCESS);
        let text = String::from_utf8_lossy(&stderr);
        assert!(text.contains("flake.nix"), "stderr: {text:?}");
        assert!(
            !root.join(".bloomery/config.toml").exists(),
            "read-only config must not create configuration"
        );

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_args_at(
            ["bloomery", "config", "--help"],
            &root,
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(status, ExitCode::SUCCESS);
        assert!(String::from_utf8_lossy(&stdout).contains("Usage"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-015"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-018"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-OUTPUT-009"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-OUTPUT-011"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-OUTPUT-009"))]
    fn mutating_config_creates_missing_configuration_with_warning() {
        let root = fixture_root("config-create");
        fs::create_dir_all(&root).expect("workspace root");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_args_at(
            [
                "bloomery",
                "config",
                "set",
                "checks.enable",
                "false",
                "--json",
            ],
            &root,
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(status, ExitCode::SUCCESS);
        assert!(stderr.is_empty());
        let document: Value = serde_json::from_slice(&stdout).expect("config JSON");
        assert_eq!(document["command"], "config");
        assert_eq!(document["status"], "succeeded");
        let warnings = document["warnings"].as_array().expect("warnings");
        assert!(
            warnings
                .iter()
                .any(|warning| warning["code"] == "missing_flake"),
            "warnings: {warnings:?}"
        );
        assert!(
            warnings
                .iter()
                .any(|warning| warning["code"] == "created_config"),
            "warnings: {warnings:?}"
        );
        assert!(root.join(".bloomery/config.toml").is_file());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-COMMANDS-001"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-COMMANDS-002"))]
    fn config_get_parses_a_key() {
        assert_eq!(
            parsed_config(&["bloomery", "config", "get", "checks.enable"]),
            ConfigOperation::Get {
                key: "checks.enable".into()
            }
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-COMMANDS-003"))]
    fn config_set_parses_a_key_and_value() {
        assert_eq!(
            parsed_config(&["bloomery", "config", "set", "checks.enable", "false"]),
            ConfigOperation::Set {
                key: "checks.enable".into(),
                value: "false".into()
            }
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-COMMANDS-004"))]
    fn config_unset_parses_a_key() {
        assert_eq!(
            parsed_config(&["bloomery", "config", "unset", "checks.enable"]),
            ConfigOperation::Unset {
                key: "checks.enable".into()
            }
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-COMMANDS-005"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-010"))]
    fn config_list_parses_a_prefix() {
        assert_eq!(
            parsed_config(&["bloomery", "config", "list", "--prefix", "checks"]),
            ConfigOperation::List {
                prefix: Some("checks".into())
            }
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-COMMANDS-006"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-011"))]
    fn config_upgrade_parses_dry_run() {
        assert_eq!(
            parsed_config(&["bloomery", "config", "upgrade", "--dry-run"]),
            ConfigOperation::Upgrade {
                dry_run: true,
                diff: false
            }
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-013"))]
    fn config_upgrade_parses_recommendation_diff() {
        assert_eq!(
            parsed_config(&["bloomery", "config", "upgrade", "--diff"]),
            ConfigOperation::Upgrade {
                dry_run: false,
                diff: true
            }
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-COMMANDS-014"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-COMMANDS-019"))]
    fn config_document_parses_without_arguments() {
        assert_eq!(
            parsed_config(&["bloomery", "config", "document"]),
            ConfigOperation::Document
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-012"))]
    #[cfg_attr(any(), bloomery("CLI-INTERFACE-FLAGS-014"))]
    fn config_only_flags_are_rejected_elsewhere() {
        assert!(
            Cli::try_parse_from(["bloomery", "config", "upgrade", "--dry-run", "--diff"]).is_err()
        );
        assert!(
            Cli::try_parse_from([
                "bloomery",
                "config",
                "get",
                "checks.enable",
                "--prefix",
                "x"
            ])
            .is_err()
        );
        assert!(Cli::try_parse_from(["bloomery", "review", "--prefix", "x"]).is_err());
        assert!(Cli::try_parse_from(["bloomery", "sync", "--diff"]).is_err());
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-OUTPUT-001"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-OUTPUT-002"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-OUTPUT-003"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-OUTPUT-004"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-OUTPUT-005"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-OUTPUT-010"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-OUTPUT-012"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-DOCUMENT-010"))]
    fn config_json_contracts() {
        let root = fixture_root("config-json");
        fs::create_dir_all(root.join(".bloomery")).expect("workspace root");
        fs::write(root.join("flake.nix"), "{ }\n").expect("flake");
        fs::write(
            root.join(".bloomery/config.toml"),
            "version = 2\nchecks.enable = false\n",
        )
        .expect("config");

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_args_at(
            ["bloomery", "config", "get", "checks.enable", "--json"],
            &root,
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(status, ExitCode::SUCCESS);
        let document: Value = serde_json::from_slice(&stdout).expect("get JSON");
        assert_eq!(document["command"], "config");
        assert_eq!(document["status"], "succeeded");
        assert_eq!(document["operation"], "get");
        assert_eq!(document["result"]["key"]["configured"], true);
        assert_eq!(document["result"]["key"]["value"], false);

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        run_args_at(
            ["bloomery", "config", "list", "--json"],
            &root,
            &mut stdout,
            &mut stderr,
        );
        let document: Value = serde_json::from_slice(&stdout).expect("list JSON");
        assert_eq!(document["operation"], "list");
        assert!(
            document["result"]["keys"]
                .as_array()
                .is_some_and(|keys| !keys.is_empty())
        );

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        run_args_at(
            [
                "bloomery",
                "config",
                "set",
                "checks.lockFreshness.enable",
                "true",
                "--json",
            ],
            &root,
            &mut stdout,
            &mut stderr,
        );
        let document: Value = serde_json::from_slice(&stdout).expect("set JSON");
        assert_eq!(document["operation"], "set");
        assert_eq!(document["result"]["changed"], true);

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        run_args_at(
            [
                "bloomery",
                "config",
                "unset",
                "checks.lockFreshness.enable",
                "--json",
            ],
            &root,
            &mut stdout,
            &mut stderr,
        );
        let document: Value = serde_json::from_slice(&stdout).expect("unset JSON");
        assert_eq!(document["operation"], "unset");
        assert_eq!(document["result"]["removed"], true);

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        run_args_at(
            ["bloomery", "config", "upgrade", "--diff", "--json"],
            &root,
            &mut stdout,
            &mut stderr,
        );
        let document: Value = serde_json::from_slice(&stdout).expect("diff JSON");
        assert_eq!(document["operation"], "upgrade");
        assert_eq!(document["result"]["written"], false);
        assert!(
            document["result"]["differences"]
                .as_array()
                .is_some_and(|differences| !differences.is_empty())
        );

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        run_args_at(
            ["bloomery", "config", "document", "--json"],
            &root,
            &mut stdout,
            &mut stderr,
        );
        let document: Value = serde_json::from_slice(&stdout).expect("document JSON");
        assert_eq!(document["operation"], "document");
        assert!(
            document["result"]["documented"]
                .as_array()
                .is_some_and(|keys| keys.iter().any(|key| key == "checks.enable"))
        );
        assert_eq!(document["result"]["written"], true);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-OUTPUT-005"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-OUTPUT-010"))]
    fn upgrade_format_transitions_are_reported_in_human_and_json_results() {
        let report = ConfigReport {
            warnings: Vec::new(),
            outcome: ConfigOutcome::Upgrade {
                source_version: Some(1),
                target_version: Some(2),
                added: vec!["checks.enable".to_owned()],
                written: false,
                differences: Vec::new(),
            },
        };

        let mut stdout = Vec::new();
        render_config_outcome(&report, output::ColorMode::Never, &mut stdout);
        let human = String::from_utf8(stdout).expect("human output");
        assert!(human.contains("format migration: 1 -> 2"), "{human}");
        assert!(human.contains("added checks.enable"), "{human}");
        assert!(human.contains("dry run; not written"), "{human}");

        let json = super::commands::config::config_success_json(&report);
        assert_eq!(json["result"]["source_version"], 1);
        assert_eq!(json["result"]["target_version"], 2);
        assert_eq!(json["result"]["written"], false);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-OUTPUT-006"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-OUTPUT-007"))]
    #[cfg_attr(any(), bloomery("CLI-CONFIG-OUTPUT-008"))]
    fn config_exit_codes() {
        let root = fixture_root("config-exit");
        fs::create_dir_all(root.join(".bloomery")).expect("workspace root");
        fs::write(root.join("flake.nix"), "{ }\n").expect("flake");
        fs::write(
            root.join(".bloomery/config.toml"),
            "version = 2\nchecks.enable = true\n",
        )
        .expect("config");

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_args_at(
            ["bloomery", "config", "get", "checks.enable", "--json"],
            &root,
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(status, ExitCode::SUCCESS);

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_args_at(
            ["bloomery", "config", "get", "checks.bogus", "--json"],
            &root,
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(status, ExitCode::from(2));
        let document: Value = serde_json::from_slice(&stdout).expect("usage JSON");
        assert_eq!(document["error"]["kind"], "usage");

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_args_at(
            [
                "bloomery",
                "config",
                "set",
                "scanners.playwright.tag_prefix",
                "",
                "--json",
            ],
            &root,
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(status, ExitCode::from(1));
        let document: Value = serde_json::from_slice(&stdout).expect("failure JSON");
        assert_eq!(document["status"], "failed");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-INIT-OUTPUT-001"))]
    fn init_json_reports_the_target_template_and_created_paths() {
        let root = fixture_root("init-json");
        let request = bloomery_init::InitRequest {
            directory: root.join("app"),
            template: Some("basic".to_owned()),
            force: false,
        };
        let mut stage = InitLock::default();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = super::run_init_request(
            &request,
            &mut stage,
            true,
            super::output::ColorMode::Auto,
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(status, ExitCode::SUCCESS);
        let document: Value = serde_json::from_slice(&stdout).expect("init JSON");
        assert_eq!(document["command"], "init");
        assert_eq!(document["status"], "succeeded");
        assert_eq!(document["result"]["template"], "basic");
        assert!(
            document["result"]["created"]
                .as_array()
                .is_some_and(|paths| !paths.is_empty())
        );
        let _ = fs::remove_dir_all(root);
    }
}
