mod asset;
mod bundle;
mod index;
#[cfg(test)]
mod tests;

use std::{env, error::Error, path::PathBuf, process::ExitCode};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn run(args: &[PathBuf]) -> Result<()> {
    match args {
        [command, crate_name, manifest_dir, out, known, artifacts @ ..] if command == "index" => {
            let known = (!known.as_os_str().is_empty()).then_some(known.as_path());
            index::build(
                crate_name.to_str().ok_or("crate name must be UTF-8")?,
                manifest_dir,
                out,
                known,
                artifacts,
            )
        }
        [command, out, sources @ ..] if command == "merge" => index::merge(out, sources),
        [command, index, artifact, output, other @ ..] if command == "bundle" => {
            bundle::write(index, artifact, std::iter::once(output).chain(other))
        }
        _ => Err("expected index CRATE MANIFEST_DIR INDEX KNOWN ARTIFACT..., merge INDEX SOURCE..., or bundle INDEX ARTIFACT OUTPUT...".into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<_> = env::args_os().skip(1).map(PathBuf::from).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
