use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl CommandOutput {
    pub fn succeeded(&self) -> bool {
        self.code == Some(0)
    }

    pub fn status_description(&self) -> String {
        match self.code {
            Some(code) => format!("exit status {code}"),
            None => "termination without an exit status".to_owned(),
        }
    }
}

/// The sync pipeline uses this boundary to preflight tools and run structured
/// subprocess arguments without invoking a shell.
pub trait CommandRunner {
    fn is_available(&self, executable: &str) -> bool;

    fn run(
        &mut self,
        executable: &OsStr,
        arguments: &[OsString],
        working_directory: &Path,
    ) -> io::Result<CommandOutput>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn is_available(&self, executable: &str) -> bool {
        executable_candidates(executable).iter().any(|path| {
            if !path.is_file() {
                return false;
            }
            is_executable(path)
        })
    }

    fn run(
        &mut self,
        executable: &OsStr,
        arguments: &[OsString],
        working_directory: &Path,
    ) -> io::Result<CommandOutput> {
        let output = Command::new(executable)
            .args(arguments)
            .current_dir(working_directory)
            .output()?;
        Ok(CommandOutput {
            code: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}

fn executable_candidates(executable: &str) -> Vec<PathBuf> {
    let Some(search_path) = std::env::var_os("PATH") else {
        return Vec::new();
    };
    let names = executable_names(executable);
    std::env::split_paths(&search_path)
        .flat_map(|directory| names.iter().map(move |name| directory.join(name)))
        .collect()
}

#[cfg(windows)]
fn executable_names(executable: &str) -> Vec<OsString> {
    let has_extension = Path::new(executable).extension().is_some();
    if has_extension {
        return vec![OsString::from(executable)];
    }
    let extensions =
        std::env::var_os("PATHEXT").unwrap_or_else(|| OsString::from(".COM;.EXE;.BAT;.CMD"));
    let mut names = vec![OsString::from(executable)];
    for extension in extensions.to_string_lossy().split(';') {
        names.push(OsString::from(format!("{executable}{extension}")));
    }
    names
}

#[cfg(not(windows))]
fn executable_names(executable: &str) -> Vec<OsString> {
    vec![OsString::from(executable)]
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::{CommandOutput, CommandRunner, SystemCommandRunner};
    use bloomery_test_macros::bloomery;

    #[test]
    fn command_output_checks_exit_status_and_keeps_streams() {
        let output = CommandOutput {
            code: Some(0),
            stdout: b"ok".to_vec(),
            stderr: b"warning".to_vec(),
        };
        assert!(output.succeeded());
        assert_eq!(output.stdout, b"ok");
        assert_eq!(output.stderr, b"warning");
        assert!(
            !CommandOutput {
                code: Some(1),
                stdout: Vec::new(),
                stderr: Vec::new(),
            }
            .succeeded()
        );
    }

    #[test]
    #[bloomery("CLI-SYNC-LOCKS-031")]
    fn system_runner_checks_tool_availability_without_running_it() {
        let runner = SystemCommandRunner;
        assert!(runner.is_available("sh"));
        assert!(!runner.is_available("bloomery-tool-that-does-not-exist"));
    }
}
