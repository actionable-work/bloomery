mod output;

use bloomery_model::{render_diagnostics, sort_diagnostics};
use clap::{ArgAction, Parser, Subcommand};
use serde_json::{Value, json};
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "bloomery",
    about = "Requirements traceability, review, and workspace synchronization"
)]
struct Cli {
    /// Emit machine-readable JSON instead of human-readable output.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate workspace structure, requirement records, and static evidence.
    Check,
    /// Print requirements that require human review.
    Review,
    /// Reconcile Cargo and Bloomery locks, optionally updating dependencies.
    Sync {
        /// Update all applicable ecosystems, or a comma-separated list of nix and rust.
        #[arg(
            long,
            value_name = "LIST",
            num_args = 0..=1,
            default_missing_value = "__bloomery_bare_update__",
            action = ArgAction::Set
        )]
        update: Option<String>,
    },
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => return report_parse_error(error),
    };
    match cli.command {
        Command::Sync { update } => run_sync(update.as_deref(), cli.json),
        command => run_workspace_command(command, cli.json),
    }
}

fn report_parse_error(error: clap::Error) -> ExitCode {
    let exit_code = error.exit_code() as u8;
    if matches!(
        error.kind(),
        clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
    ) {
        let _ = error.print();
        return ExitCode::from(exit_code);
    }

    let arguments = std::env::args_os().collect::<Vec<_>>();
    if arguments
        .iter()
        .any(|argument| argument == "--json" || argument.to_string_lossy().starts_with("--json="))
    {
        let command = command_from_arguments(&arguments);
        let document = output::usage_error_json(command, &error.to_string());
        if print_json(&document).is_err() {
            eprintln!("bloomery: unable to render JSON usage error");
            return ExitCode::from(1);
        }
    } else {
        let _ = error.print();
    }
    ExitCode::from(exit_code)
}

fn command_from_arguments(arguments: &[OsString]) -> Option<&'static str> {
    arguments
        .iter()
        .find_map(|argument| match argument.to_str()? {
            "check" => Some("check"),
            "review" => Some("review"),
            "sync" => Some("sync"),
            _ => None,
        })
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

fn run_workspace_command(command: Command, json_mode: bool) -> ExitCode {
    let name = match command {
        Command::Check => "check",
        Command::Review => "review",
        Command::Sync { .. } => unreachable!("sync is dispatched before workspace loading"),
    };
    let root = match std::env::current_dir() {
        Ok(root) => root,
        Err(error) => return report_runtime_error(name, &error.to_string(), json_mode),
    };
    let context = match bloomery_workspace::load(&root) {
        Ok(context) => context,
        Err(mut diagnostics) => {
            sort_diagnostics(&mut diagnostics);
            if json_mode {
                let document = output::diagnostics_json(name, "failed", &root, &diagnostics);
                if print_json(&document).is_err() {
                    eprintln!("bloomery: unable to render JSON diagnostics");
                    return ExitCode::from(1);
                }
            } else {
                let rendered = render_diagnostics(&root, &diagnostics);
                eprint!(
                    "{}",
                    output::colorize_check(
                        &rendered,
                        output::color_enabled(output::Stream::Stderr, false)
                    )
                );
            }
            return ExitCode::from(1);
        }
    };

    match command {
        Command::Check => match bloomery_check::run(&context) {
            Ok(()) if json_mode => {
                let document = output::diagnostics_json("check", "passed", &root, &[]);
                if print_json(&document).is_err() {
                    eprintln!("bloomery: unable to render JSON check result");
                    ExitCode::from(1)
                } else {
                    ExitCode::SUCCESS
                }
            }
            Ok(()) => {
                let message = "PASS: traceability verification succeeded.\n";
                print!(
                    "{}",
                    output::colorize_check(
                        message,
                        output::color_enabled(output::Stream::Stdout, false)
                    )
                );
                ExitCode::SUCCESS
            }
            Err(mut diagnostics) => {
                sort_diagnostics(&mut diagnostics);
                if json_mode {
                    let document = output::diagnostics_json("check", "failed", &root, &diagnostics);
                    if print_json(&document).is_err() {
                        eprintln!("bloomery: unable to render JSON diagnostics");
                        ExitCode::from(1)
                    } else {
                        ExitCode::from(1)
                    }
                } else {
                    let rendered = render_diagnostics(&root, &diagnostics);
                    eprint!(
                        "{}",
                        output::colorize_check(
                            &rendered,
                            output::color_enabled(output::Stream::Stderr, false)
                        )
                    );
                    ExitCode::from(1)
                }
            }
        },
        Command::Review => {
            let items = bloomery_review::items(&context);
            if json_mode {
                match serde_json::to_value(&items) {
                    Ok(document) => {
                        if print_json(&document).is_err() {
                            eprintln!("bloomery: unable to render JSON review catalog");
                            ExitCode::from(1)
                        } else {
                            ExitCode::SUCCESS
                        }
                    }
                    Err(error) => {
                        let document = output::usage_error_json(Some("review"), &error.to_string());
                        let _ = print_json(&document);
                        ExitCode::from(1)
                    }
                }
            } else {
                let rendered = bloomery_review::render_text(&items);
                print!(
                    "{}",
                    output::colorize_review(
                        &rendered,
                        output::color_enabled(output::Stream::Stdout, false)
                    )
                );
                ExitCode::SUCCESS
            }
        }
        Command::Sync { .. } => unreachable!("sync is dispatched before workspace loading"),
    }
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
    use super::{Cli, Command};
    use bloomery_test_macros::bloomery;
    use clap::Parser;

    fn parse_update(args: &[&str]) -> Result<Option<String>, clap::Error> {
        match Cli::try_parse_from(args)?.command {
            Command::Sync { update } => Ok(update),
            _ => panic!("expected sync command"),
        }
    }

    fn parse_json(args: &[&str]) -> Result<bool, clap::Error> {
        Cli::try_parse_from(args).map(|cli| cli.json)
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
    #[bloomery("CLI-INTERFACE-OUTPUT-003")]
    #[bloomery("CLI-INTERFACE-FLAGS-006")]
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
}
