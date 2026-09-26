use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(name = "cli")]
#[command(about = "CLI tool to interact with axum server")]
struct Args {
    #[arg(short, long, default_value = "http://127.0.0.1:3000")]
    url: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(about = "Check health of the server")]
    Health,

    #[command(about = "Send a greeting request")]
    Greet {
        #[arg(help = "Name to greet")]
        name: String,
    },

    #[command(about = "Send an echo request")]
    Echo {
        #[arg(help = "Message to echo")]
        message: String,
    },
}

#[derive(Serialize, Deserialize, Debug)]
struct StatusResponse {
    status: String,
    service: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct GreetResponse {
    message: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct EchoPayload {
    text: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct EchoResponse {
    echo: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let base_url = args.url.trim_end_matches('/');

    match args.command {
        Commands::Health => {
            let res: StatusResponse = ureq::get(&format!("{}/health", base_url))
                .call()?
                .into_json()?;
            println!("Status: {} (Service: {})", res.status, res.service);
        }
        Commands::Greet { name } => {
            let res: GreetResponse = ureq::get(&format!("{}/greet/{}", base_url, name))
                .call()?
                .into_json()?;
            println!("{}", res.message);
        }
        Commands::Echo { message } => {
            let payload = EchoPayload { text: message };
            let res: EchoResponse = ureq::post(&format!("{}/echo", base_url))
                .send_json(payload)?
                .into_json()?;
            println!("Echo: {}", res.echo);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        let args =
            Args::try_parse_from(["cli", "--url", "http://localhost:8080", "health"]).unwrap();
        assert_eq!(args.url, "http://localhost:8080");
        match args.command {
            Commands::Health => {}
            _ => panic!("expected health command"),
        }
    }
}
