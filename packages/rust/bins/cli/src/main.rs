use bloomery_model::{render_diagnostics, sort_diagnostics};
use clap::{Parser, Subcommand, ValueEnum};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "bloomery",
    about = "Offline static requirements traceability and validation"
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
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
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
    match cli.command {
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
    }
}
