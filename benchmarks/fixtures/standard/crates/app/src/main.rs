use clap::Parser;

#[derive(Parser)]
struct Args {
    /// Name to greet.
    #[arg(long, default_value = "bench")]
    name: String,
}

fn main() {
    let args = Args::parse();
    let _router = server::router();
    println!(
        "{} {} {}",
        args.name,
        shared::label(leaf::answer()),
        leaf::answer()
    );
}
