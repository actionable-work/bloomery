use bloomery_model::{render_diagnostics, sort_diagnostics};
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "bloomery",
    about = "Requirements traceability, review, and workspace synchronization"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate workspace structure, requirement records, and static evidence.
    Check,
    /// Print requirements that require human review.
    Review {
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
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

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Sync { update } => run_sync(update.as_deref()),
        command => run_workspace_command(command),
    }
}

fn run_sync(update: Option<&str>) -> ExitCode {
    let selection = match bloomery_sync::parse_cli_update(update) {
        Ok(selection) => selection,
        Err(error) => {
            eprintln!("error: {error}\nUsage: bloomery sync [--update[=nix,rust]]");
            return ExitCode::from(error.exit_code());
        }
    };
    let root = match std::env::current_dir() {
        Ok(root) => root,
        Err(error) => {
            eprintln!("bloomery: unable to determine workspace root: {error}");
            return ExitCode::from(1);
        }
    };
    let mut runner = bloomery_sync::SystemCommandRunner;
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    match bloomery_sync::run(
        &root,
        &selection,
        &mut runner,
        &mut stdout.lock(),
        &mut stderr.lock(),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}

fn run_workspace_command(command: Command) -> ExitCode {
    let root = match std::env::current_dir() {
        Ok(root) => root,
        Err(error) => {
            eprintln!("bloomery: unable to determine repository root: {error}");
            return ExitCode::from(1);
        }
    };
    let context = match bloomery_workspace::load(&root) {
        Ok(context) => context,
        Err(mut diagnostics) => {
            sort_diagnostics(&mut diagnostics);
            eprint!("{}", render_diagnostics(&root, &diagnostics));
            return ExitCode::from(1);
        }
    };
    match command {
        Command::Check => match bloomery_check::run(&context) {
            Ok(()) => {
                println!("PASS: traceability verification succeeded.");
                ExitCode::SUCCESS
            }
            Err(mut diagnostics) => {
                sort_diagnostics(&mut diagnostics);
                eprint!("{}", render_diagnostics(&root, &diagnostics));
                ExitCode::from(1)
            }
        },
        Command::Review { format } => {
            let items = bloomery_review::items(&context);
            match format {
                OutputFormat::Text => print!("{}", bloomery_review::render_text(&items)),
                OutputFormat::Json => match serde_json::to_string_pretty(&items) {
                    Ok(json) => println!("{json}"),
                    Err(error) => {
                        eprintln!("bloomery: unable to render JSON: {error}");
                        return ExitCode::from(1);
                    }
                },
            }
            ExitCode::SUCCESS
        }
        Command::Sync { .. } => unreachable!("sync is dispatched before workspace loading"),
    }
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
}
