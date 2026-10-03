use bloomery_check_command as check_command;
use bloomery_cli_output as output;
use bloomery_cli_parser::{ParseError, ParseErrorKind, parse_from};
use bloomery_cli_types::{CliCommand, CliInvocation};
use bloomery_model::{render_diagnostics, sort_diagnostics};
use serde_json::{Value, json};
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

/// Parse and execute the CLI request using the current process environment.
pub fn run() -> ExitCode {
    match parse_from(std::env::args_os()) {
        Ok(invocation) => run_invocation(invocation),
        Err(error) => report_parse_error(error),
    }
}

fn run_invocation(invocation: CliInvocation) -> ExitCode {
    match invocation.command {
        CliCommand::Check(args) => check_command::run(args, invocation.json_mode),
        CliCommand::Sync { update } => run_sync(update.as_deref(), invocation.json_mode),
        command => run_workspace_command(command, invocation.json_mode),
    }
}

fn report_parse_error(error: ParseError) -> ExitCode {
    let exit_code = error.exit_code();
    if matches!(
        error.kind(),
        ParseErrorKind::DisplayHelp | ParseErrorKind::DisplayVersion
    ) {
        let _ = io::stdout().write_all(error.message().as_bytes());
        return ExitCode::from(exit_code);
    }

    if error.json_mode() {
        if error.command() == Some("check") {
            let document =
                check_command::check_output::error_document("UsageError", error.message());
            if print_json(&document).is_err() {
                eprintln!("bloomery: unable to render JSON usage error");
                return ExitCode::from(1);
            }
        } else {
            let document = output::usage_error_json(error.command(), error.message());
            if print_json(&document).is_err() {
                eprintln!("bloomery: unable to render JSON usage error");
                return ExitCode::from(1);
            }
        }
    } else {
        let _ = io::stderr().write_all(error.message().as_bytes());
    }
    ExitCode::from(exit_code)
}

fn run_sync(update: Option<&str>, json_mode: bool) -> ExitCode {
    let selection = match bloomery_sync::parse_cli_update(update) {
        Ok(selection) => selection,
        Err(error) => {
            if json_mode {
                let document = output::usage_error_json(Some("sync"), &error.to_string());
                if print_json(&document).is_err() {
                    eprintln!("bloomery: unable to render JSON usage error");
                    return ExitCode::from(1);
                }
            } else {
                eprintln!(
                    "{}",
                    output::colorize_error(
                        &format!("error: {error}\nUsage: bloomery sync [--update[=nix,rust]]"),
                        output::color_enabled(output::Stream::Stderr, false)
                    )
                );
            }
            return ExitCode::from(error.exit_code());
        }
    };
    let root = match std::env::current_dir() {
        Ok(root) => root,
        Err(error) => {
            return report_runtime_error("sync", &error.to_string(), json_mode);
        }
    };

    let mut runner = bloomery_sync::SystemCommandRunner;
    if json_mode {
        let mut progress = Vec::new();
        let mut diagnostics = Vec::new();
        return match bloomery_sync::run(
            &root,
            &selection,
            &mut runner,
            &mut progress,
            &mut diagnostics,
        ) {
            Ok(report) => {
                if print_json(&output::sync_success_json(&report)).is_err() {
                    eprintln!("bloomery: unable to render JSON sync result");
                    ExitCode::from(1)
                } else {
                    ExitCode::SUCCESS
                }
            }
            Err(error) => {
                let exit_code = error.exit_code();
                let document = output::sync_failure_json(
                    error.stage(),
                    error.message(),
                    error.completed_stages(),
                    error.may_be_partially_synchronized(),
                    &String::from_utf8_lossy(&diagnostics),
                );
                if print_json(&document).is_err() {
                    eprintln!("bloomery: unable to render JSON sync error");
                    ExitCode::from(1)
                } else {
                    ExitCode::from(exit_code)
                }
            }
        };
    }

    let mut stdout = output::ColorWriter::stdout(
        io::stdout().lock(),
        output::color_enabled(output::Stream::Stdout, false),
    );
    let mut stderr = output::ColorWriter::stderr(
        io::stderr().lock(),
        output::color_enabled(output::Stream::Stderr, false),
    );
    let result = bloomery_sync::run(&root, &selection, &mut runner, &mut stdout, &mut stderr);
    let _ = stdout.flush();
    let _ = stderr.flush();
    drop(stdout);
    drop(stderr);
    match result {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!(
                "{}",
                output::colorize_error(
                    &format!("error: {error}"),
                    output::color_enabled(output::Stream::Stderr, false)
                )
            );
            ExitCode::from(error.exit_code())
        }
    }
}

fn run_workspace_command(command: CliCommand, json_mode: bool) -> ExitCode {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    run_workspace_command_from_current_dir(command, json_mode, &mut stdout, &mut stderr)
}

fn run_workspace_command_from_current_dir(
    command: CliCommand,
    json_mode: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let name = match &command {
        CliCommand::Check(_) => "check",
        CliCommand::Review => "review",
        CliCommand::Sync { .. } => unreachable!("sync is dispatched before workspace loading"),
    };
    let root = match std::env::current_dir() {
        Ok(root) => root,
        Err(error) => return report_runtime_error(name, &error.to_string(), json_mode),
    };
    run_workspace_command_at(command, json_mode, &root, stdout, stderr)
}

fn run_workspace_command_at(
    command: CliCommand,
    json_mode: bool,
    root: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let command = match command {
        CliCommand::Check(args) => {
            return check_command::run_at(args, json_mode, root, stdout, stderr);
        }
        CliCommand::Review => CliCommand::Review,
        CliCommand::Sync { .. } => unreachable!("sync is dispatched before workspace loading"),
    };
    let name = "review";
    let context = match bloomery_workspace::load(root) {
        Ok(context) => context,
        Err(mut diagnostics) => {
            sort_diagnostics(&mut diagnostics);
            if json_mode {
                let document = output::diagnostics_json(name, "failed", root, &diagnostics);
                if write_json(stdout, &document).is_err() {
                    let _ = writeln!(stderr, "bloomery: unable to render JSON diagnostics");
                    return ExitCode::from(1);
                }
            } else {
                let rendered = render_diagnostics(root, &diagnostics);
                let rendered = output::colorize_check(
                    &rendered,
                    output::color_enabled(output::Stream::Stderr, false),
                );
                let _ = stderr.write_all(rendered.as_bytes());
            }
            return ExitCode::from(1);
        }
    };

    match command {
        CliCommand::Review => {
            let items = bloomery_review::items(&context);
            if json_mode {
                match serde_json::to_value(&items) {
                    Ok(document) => {
                        if write_json(stdout, &document).is_err() {
                            let _ =
                                writeln!(stderr, "bloomery: unable to render JSON review catalog");
                            ExitCode::from(1)
                        } else {
                            ExitCode::SUCCESS
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
                let rendered = output::colorize_review(
                    &rendered,
                    output::color_enabled(output::Stream::Stdout, false),
                );
                let _ = stdout.write_all(rendered.as_bytes());
                ExitCode::SUCCESS
            }
        }
        CliCommand::Check(_) | CliCommand::Sync { .. } => {
            unreachable!("non-review command was dispatched before workspace loading")
        }
    }
}

#[cfg(test)]
fn run_workspace_command_at_with_check_cache(
    command: CliCommand,
    json_mode: bool,
    root: &Path,
    cache_base: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    match command {
        CliCommand::Check(args) => {
            check_command::run_at_with_cache(args, json_mode, root, cache_base, stdout, stderr)
        }
        command => run_workspace_command_at(command, json_mode, root, stdout, stderr),
    }
}

fn write_json(writer: &mut impl Write, value: &Value) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *writer, value).map_err(io::Error::other)?;
    writer.write_all(b"\n")
}

fn report_runtime_error(command: &str, message: &str, json_mode: bool) -> ExitCode {
    if json_mode {
        let document = json!({
            "command": command,
            "status": "failed",
            "error": { "kind": "runtime", "message": message },
        });
        if print_json(&document).is_err() {
            eprintln!("bloomery: unable to render JSON error");
            return ExitCode::from(1);
        }
    } else {
        eprintln!(
            "{}",
            output::colorize_error(
                &format!("error: bloomery: {message}"),
                output::color_enabled(output::Stream::Stderr, false)
            )
        );
    }
    ExitCode::from(1)
}

fn print_json(value: &Value) -> Result<(), serde_json::Error> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        check_command, run_workspace_command_at, run_workspace_command_at_with_check_cache,
        run_workspace_command_from_current_dir,
    };
    use bloomery_cli_parser::{ParseError, ParseErrorKind as ErrorKind, parse_from};
    use bloomery_cli_types::CliCommand as Command;
    use bloomery_test_macros::bloomery;
    use serde_json::Value;
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;
    use std::process::ExitCode;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[derive(Debug)]
    struct Cli {
        json: bool,
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
    #[bloomery("REPOSITORY-LAYOUT-STRUCTURE-007")]
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
    #[bloomery("REPOSITORY-LAYOUT-STRUCTURE-008")]
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

    fn fixture_root(name: &str) -> PathBuf {
        let suffix = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "bloomery-cli-{name}-{}-{suffix}",
            std::process::id()
        ))
    }

    fn fixture_workspace(requirement_id: &str, manual: bool) -> PathBuf {
        let root = fixture_root("workspace");
        let area = root.join(".bloomery/specs/CLI");
        let feature = area.join("CHECK");
        fs::create_dir_all(feature.join("design")).expect("design directory");
        fs::create_dir_all(feature.join("requirements")).expect("requirements directory");
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
    #[bloomery("CLI-SYNC-INTERFACE-005")]
    fn clap_accepts_equals_update_syntax() {
        assert_eq!(
            parse_update(&["bloomery", "sync", "--update=nix,rust"])
                .expect("equals syntax")
                .as_deref(),
            Some("nix,rust")
        );
    }

    #[test]
    #[bloomery("CLI-SYNC-INTERFACE-006")]
    fn clap_accepts_a_separate_update_argument() {
        assert_eq!(
            parse_update(&["bloomery", "sync", "--update", "nix,rust"])
                .expect("separate argument")
                .as_deref(),
            Some("nix,rust")
        );
    }

    #[test]
    #[bloomery("CLI-SYNC-INTERFACE-008")]
    #[bloomery("CLI-SYNC-INTERFACE-009")]
    #[bloomery("CLI-SYNC-INTERFACE-010")]
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
    #[bloomery("CLI-SYNC-INTERFACE-015")]
    fn clap_rejects_repeated_update_flags_with_usage_exit_code() {
        let error = parse_update(&["bloomery", "sync", "--update=rust", "--update=nix"])
            .expect_err("duplicate update flags");
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    #[bloomery("CLI-SYNC-INTERFACE-023")]
    fn cli_does_not_expose_a_lock_subcommand() {
        let error =
            Cli::try_parse_from(["bloomery", "lock"]).expect_err("lock must not be a CLI command");
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    #[bloomery("CLI-INTERFACE-COMMANDS-001")]
    #[bloomery("CLI-INTERFACE-COMMANDS-002")]
    #[bloomery("CLI-INTERFACE-COMMANDS-003")]
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
    }

    #[test]
    #[bloomery("CLI-INTERFACE-COMMANDS-004")]
    fn clap_provides_its_help_subcommand() {
        for args in [vec!["bloomery", "help"], vec!["bloomery", "help", "check"]] {
            let error = Cli::try_parse_from(args).expect_err("help should be rendered by clap");
            assert_eq!(error.kind(), ErrorKind::DisplayHelp);
            assert_eq!(error.exit_code(), 0);
            assert!(error.to_string().contains("Usage: bloomery"));
        }
    }

    #[test]
    #[bloomery("CLI-INTERFACE-COMMANDS-005")]
    fn repository_commands_resolve_against_the_process_working_directory() {
        let _lock = CURRENT_DIR_LOCK.lock().expect("working-directory lock");
        let root = fixture_root("cwd");
        fs::create_dir_all(&root).expect("workspace root");
        let previous = std::env::current_dir().expect("original working directory");
        std::env::set_current_dir(&root).expect("set working directory");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_workspace_command_from_current_dir(
            Command::Review,
            false,
            &mut stdout,
            &mut stderr,
        );
        std::env::set_current_dir(previous).expect("restore working directory");

        assert_eq!(status, ExitCode::FAILURE);
        assert!(
            String::from_utf8(stderr)
                .expect("diagnostics")
                .contains("MissingSpecsDirectory")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[bloomery("CLI-INTERFACE-FLAGS-001")]
    fn review_defaults_to_human_readable_output() {
        let root = fixture_workspace("CLI-CHECK-CONTRACT-001", true);
        let json_mode = parse_json(&["bloomery", "review"]).expect("default output mode");
        assert!(!json_mode);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status =
            run_workspace_command_at(Command::Review, json_mode, &root, &mut stdout, &mut stderr);

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
    #[bloomery("CLI-INTERFACE-FLAGS-002")]
    fn review_emits_json_when_requested() {
        let root = fixture_workspace("CLI-CHECK-CONTRACT-001", true);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status =
            run_workspace_command_at(Command::Review, true, &root, &mut stdout, &mut stderr);

        assert_eq!(status, ExitCode::SUCCESS);
        assert!(stderr.is_empty());
        let items: Vec<Value> = serde_json::from_slice(&stdout).expect("review JSON array");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "CLI-CHECK-CONTRACT-001");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[bloomery("CLI-INTERFACE-FLAGS-003")]
    fn all_application_commands_accept_the_same_json_flag() {
        for command in [
            ["bloomery", "check", "--json"],
            ["bloomery", "review", "--json"],
            ["bloomery", "sync", "--json"],
        ] {
            assert!(parse_json(&command).expect("--json parses"));
        }
        assert!(parse_json(&["bloomery", "--json", "check"]).expect("global --json"));
        assert!(Cli::try_parse_from(["bloomery", "review", "--format", "json"]).is_err());
    }

    #[test]
    #[bloomery("CLI-INTERFACE-COMMANDS-006")]
    #[bloomery("CLI-INTERFACE-COMMANDS-007")]
    #[bloomery("CLI-INTERFACE-COMMANDS-008")]
    #[bloomery("CLI-INTERFACE-FLAGS-007")]
    #[bloomery("CLI-CHECK-SELECT-010")]
    #[bloomery("CLI-INTERFACE-FLAGS-008")]
    #[bloomery("CLI-INTERFACE-FLAGS-009")]
    fn check_exposes_nested_retrieval_options_and_rejects_execution_flags_there() {
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
            &["bloomery", "--json", "check", "list"][..],
            &["bloomery", "check", "list", "--json"][..],
        ] {
            assert!(
                Cli::try_parse_from(args).is_ok(),
                "failed to parse {args:?}"
            );
        }
        for args in [
            &["bloomery", "check", "list", "--jobs", "2"][..],
            &["bloomery", "check", "failures", "--fail-fast"][..],
            &["bloomery", "check", "details", "f1", "--check", "static:* "][..],
            &["bloomery", "check", "--jobs", "not-a-number"][..],
            &["bloomery", "check", "list", "--offset", "-1"][..],
        ] {
            assert!(
                Cli::try_parse_from(args).is_err(),
                "unexpectedly parsed {args:?}"
            );
        }
    }

    #[test]
    #[bloomery("CLI-INTERFACE-FLAGS-005")]
    fn every_application_command_accepts_short_and_long_help_flags() {
        for command in ["check", "review", "sync"] {
            for flag in ["-h", "--help"] {
                let error = Cli::try_parse_from(["bloomery", command, flag])
                    .expect_err("help flag should short-circuit parsing");
                assert_eq!(error.kind(), ErrorKind::DisplayHelp);
                assert_eq!(error.exit_code(), 0);
            }
        }
    }

    #[test]
    #[bloomery("CLI-CHECK-CONTRACT-001")]
    fn check_fails_when_workspace_structure_is_invalid() {
        let root = fixture_workspace("CLI-CHECK-CONTRACT-001", true);
        let cache = fixture_root("cache");
        fs::create_dir_all(&cache).expect("cache base");
        fs::remove_file(root.join(".bloomery/specs/CLI/CHECK/README.md"))
            .expect("remove feature README");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_workspace_command_at_with_check_cache(
            Command::Check(check_command::CheckArgs::default()),
            false,
            &root,
            &cache,
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(status, ExitCode::FAILURE);
        assert!(
            String::from_utf8(stdout)
                .expect("summary")
                .contains("MissingDocument")
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(cache);
    }

    #[test]
    #[bloomery("CLI-CHECK-CONTRACT-002")]
    fn check_fails_when_automated_traceability_is_invalid() {
        let root = fixture_workspace("CLI-CHECK-CONTRACT-002", false);
        let cache = fixture_root("cache");
        fs::create_dir_all(&cache).expect("cache base");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_workspace_command_at_with_check_cache(
            Command::Check(check_command::CheckArgs::default()),
            false,
            &root,
            &cache,
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(status, ExitCode::FAILURE);
        assert!(
            String::from_utf8(stdout)
                .expect("summary")
                .contains("MissingAutomatedTest")
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(cache);
    }

    #[test]
    #[bloomery("CLI-INTERFACE-OUTPUT-003")]
    #[bloomery("CLI-INTERFACE-FLAGS-006")]
    fn every_application_command_accepts_the_shared_json_flag_before_or_after_its_name() {
        for command in ["check", "review", "sync"] {
            assert!(parse_json(&["bloomery", command, "--json"]).expect("trailing --json"));
            assert!(parse_json(&["bloomery", "--json", command]).expect("leading --json"));
        }
        assert!(Cli::try_parse_from(["bloomery", "review", "--format", "json"]).is_err());
    }

    #[test]
    #[bloomery("CLI-INTERFACE-COMMANDS-004")]
    fn top_level_help_is_generated_by_the_parser() {
        let error = Cli::try_parse_from(["bloomery", "--help"]).expect_err("help output");
        assert_eq!(error.kind(), ErrorKind::DisplayHelp);
        assert_eq!(error.exit_code(), 0);
        assert!(error.to_string().contains("Usage: bloomery"));
    }
}
