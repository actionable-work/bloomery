use bloomery_docs_consistency::check;
use bloomery_model::render_diagnostics;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let root = root_from_arguments();
    match check(&root) {
        Ok(diagnostics) if diagnostics.is_empty() => {
            println!("documentation consistency: OK");
            ExitCode::SUCCESS
        }
        Ok(diagnostics) => {
            eprint!("{}", render_diagnostics(&root, &diagnostics));
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("documentation consistency: {error}");
            ExitCode::from(2)
        }
    }
}

fn root_from_arguments() -> PathBuf {
    let mut root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        if argument.to_string_lossy() == "--root"
            && let Some(value) = arguments.next()
        {
            root = PathBuf::from(value);
        }
    }
    root
}
