//! Strict file-backed subprocess fixture. This executable never invokes Nix.
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

fn claim(root: &Path) -> io::Result<PathBuf> {
    let lock = root.join("lock");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match fs::create_dir(&lock) {
            Ok(()) => break,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                if Instant::now() >= deadline {
                    return Err(io::Error::other("fixture call lock timed out"));
                }
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(error),
        }
    }
    let result = (|| {
        let mut calls = fs::read_dir(root)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir() && path.file_name().unwrap() != "lock")
            .collect::<Vec<_>>();
        calls.sort();
        let call = calls
            .into_iter()
            .find(|path| !path.join("started").exists())
            .ok_or_else(|| io::Error::other("unexpected extra fixture call"))?;
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(call.join("started"))?;
        Ok(call)
    })();
    fs::remove_dir(lock)?;
    result
}

fn run(root: &Path) -> io::Result<i32> {
    let call = claim(root)?;
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let encoded = args
        .iter()
        .flat_map(|arg| arg.bytes().chain([0]))
        .collect::<Vec<_>>();
    fs::write(call.join("actual-args"), &encoded)?;
    if encoded != fs::read(call.join("args"))? {
        return Err(io::Error::other(format!("unexpected argv: {args:?}")));
    }
    let cwd = fs::canonicalize(std::env::current_dir()?)?;
    if cwd != PathBuf::from(fs::read_to_string(call.join("cwd"))?) {
        return Err(io::Error::other("unexpected working directory"));
    }
    if call.join("stdin").exists() {
        let mut input = Vec::new();
        io::stdin().read_to_end(&mut input)?;
        fs::write(call.join("actual-stdin"), &input)?;
        if input != fs::read(call.join("stdin"))? {
            return Err(io::Error::other("unexpected stdin"));
        }
    }
    if call.join("edit-path").exists() {
        let path = fs::read_to_string(call.join("edit-path"))?;
        fs::write(cwd.join(path), fs::read(call.join("edit-contents"))?)?;
    }
    io::stdout().write_all(&fs::read(call.join("stdout"))?)?;
    io::stderr().write_all(&fs::read(call.join("stderr"))?)?;
    if call.join("block").exists() {
        fs::write(call.join("blocked"), b"")?;
        // The adapter must cancel this owned child. Bound the wait so regressions fail.
        thread::sleep(Duration::from_secs(10));
        return Err(io::Error::other("fixture was not canceled"));
    }
    let status = fs::read_to_string(call.join("status"))?
        .parse()
        .map_err(io::Error::other)?;
    fs::write(call.join("finished"), b"")?;
    Ok(status)
}

fn main() {
    let root = std::env::current_dir()
        .expect("fixture cwd")
        .join(".nix-fixture");
    match run(&root) {
        Ok(status) => std::process::exit(status),
        Err(error) => {
            let _ = fs::write(
                root.join(format!("error-{}", std::process::id())),
                error.to_string(),
            );
            eprintln!("Nix subprocess fixture: {error}");
            std::process::exit(91);
        }
    }
}
