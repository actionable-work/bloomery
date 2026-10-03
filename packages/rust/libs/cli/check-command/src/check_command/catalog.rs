use super::interrupt::CancellationToken;
use super::model::{Notice, valid_system_name};
use serde_json::Value;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::Duration;

const LEGACY_CHECK_ATTRIBUTE: &str = "bloomery:check";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NixTaskResult<T> {
    Succeeded(T),
    Failed { code: String, message: String },
    Canceled,
    OperationalError(String),
}

pub trait NixBackend: Send + Sync {
    fn host_system(&self, root: &Path) -> Result<String, String>;
    fn discover(&self, root: &Path, system: &str) -> Result<Vec<String>, String>;
    fn realize_check(
        &self,
        root: &Path,
        system: &str,
        attribute: &str,
        log_path: &Path,
        cancellation: CancellationToken,
    ) -> NixTaskResult<()>;
    fn read_derivation_log(&self, root: &Path, store_path: &str) -> Result<Vec<u8>, String>;
}

#[derive(Debug, Clone)]
pub struct NixCli {
    executable: OsString,
}

impl Default for NixCli {
    fn default() -> Self {
        Self {
            executable: OsString::from("nix"),
        }
    }
}

struct LoggedCommand<'a> {
    root: &'a Path,
    args: &'a [OsString],
    log_path: &'a Path,
    cancellation: CancellationToken,
    capture_stdout: bool,
    failure_code: Option<&'a str>,
    failure_description: &'a str,
}

impl NixCli {
    #[cfg(test)]
    pub fn with_executable(executable: impl Into<OsString>) -> Self {
        Self {
            executable: executable.into(),
        }
    }

    fn run_catalog_command(
        &self,
        root: &Path,
        args: &[OsString],
        operation: &str,
    ) -> Result<Vec<u8>, String> {
        let output = Command::new(&self.executable)
            .args(args)
            .current_dir(root)
            .output()
            .map_err(|error| nix_start_error(operation, error))?;
        if !output.status.success() {
            let detail = bounded_process_error(&output.stderr);
            return Err(if detail.is_empty() {
                format!("Nix {operation} failed with {}", status_text(output.status))
            } else {
                format!("Nix {operation} failed: {detail}")
            });
        }
        Ok(output.stdout)
    }

    fn read_derivation_log(&self, root: &Path, store_path: &str) -> Result<Vec<u8>, String> {
        if !valid_nix_derivation_path(store_path) {
            return Err("invalid retained Nix derivation path".to_owned());
        }
        let output = Command::new(&self.executable)
            .arg("log")
            .arg(store_path)
            .current_dir(root)
            .output()
            .map_err(|error| nix_start_error("derivation log retrieval", error))?;
        if !output.status.success() {
            let detail = bounded_process_error(&output.stderr);
            return Err(if detail.is_empty() {
                format!("Nix log failed with {}", status_text(output.status))
            } else {
                format!("Nix log failed: {detail}")
            });
        }
        let mut log = output.stdout;
        if !output.stderr.is_empty() {
            log.extend_from_slice(b"\n");
            log.extend_from_slice(&output.stderr);
        }
        Ok(log)
    }

    fn run_logged_command(&self, command: LoggedCommand<'_>) -> NixTaskResult<Vec<u8>> {
        let LoggedCommand {
            root,
            args,
            log_path,
            cancellation,
            capture_stdout,
            failure_code,
            failure_description,
        } = command;
        let mut log = match open_private_log(log_path) {
            Ok(file) => file,
            Err(error) => {
                return NixTaskResult::OperationalError(format!(
                    "unable to open retained task log: {error}"
                ));
            }
        };
        let stderr = match log.try_clone() {
            Ok(file) => file,
            Err(error) => {
                return NixTaskResult::OperationalError(format!(
                    "unable to prepare retained task log: {error}"
                ));
            }
        };

        let mut command = Command::new(&self.executable);
        command
            .args(args)
            .current_dir(root)
            .stderr(Stdio::from(stderr));
        if capture_stdout {
            command.stdout(Stdio::piped());
        } else {
            match log.try_clone() {
                Ok(file) => {
                    command.stdout(Stdio::from(file));
                }
                Err(error) => {
                    return NixTaskResult::OperationalError(format!(
                        "unable to prepare retained task log: {error}"
                    ));
                }
            }
        }

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                return NixTaskResult::OperationalError(nix_start_error_message(error));
            }
        };

        let stdout_reader = if capture_stdout {
            child.stdout.take().map(|mut stdout| {
                thread::spawn(move || {
                    let mut bytes = Vec::new();
                    let result = stdout.read_to_end(&mut bytes);
                    (bytes, result)
                })
            })
        } else {
            None
        };

        let status = loop {
            if cancellation.is_canceled() {
                let _ = child.kill();
                let _ = child.wait();
                let captured = join_stdout(stdout_reader);
                if let Some(bytes) = captured {
                    let _ = log.write_all(&bytes);
                }
                let _ = log.flush();
                return NixTaskResult::Canceled;
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => thread::sleep(Duration::from_millis(20)),
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return NixTaskResult::OperationalError(format!(
                        "unable to wait for Nix task: {error}"
                    ));
                }
            }
        };

        let stdout = match stdout_reader {
            Some(reader) => match reader.join() {
                Ok((bytes, Ok(_))) => bytes,
                Ok((_bytes, Err(error))) => {
                    return NixTaskResult::OperationalError(format!(
                        "unable to capture Nix output: {error}"
                    ));
                }
                Err(_) => {
                    return NixTaskResult::OperationalError(
                        "Nix output reader terminated unexpectedly".to_owned(),
                    );
                }
            },
            None => Vec::new(),
        };
        if log.write_all(&stdout).is_err() || log.flush().is_err() {
            return NixTaskResult::OperationalError("unable to retain Nix task output".to_owned());
        }
        if status.success() {
            NixTaskResult::Succeeded(stdout)
        } else if let Some(code) = failure_code {
            NixTaskResult::Failed {
                code: code.to_owned(),
                message: format!("{failure_description} with {}", status_text(status)),
            }
        } else {
            NixTaskResult::OperationalError(format!(
                "{failure_description} with {}",
                status_text(status)
            ))
        }
    }
}

impl NixBackend for NixCli {
    fn host_system(&self, _root: &Path) -> Result<String, String> {
        host_nix_system()
    }

    fn discover(&self, root: &Path, system: &str) -> Result<Vec<String>, String> {
        if !valid_system_name(system) {
            return Err(format!("invalid Nix system name '{system}'"));
        }
        let output = self.run_catalog_command(root, &discover_args(system), "check discovery")?;
        let value: Value = serde_json::from_slice(&output)
            .map_err(|error| format!("Nix returned invalid check catalog JSON: {error}"))?;
        value
            .as_array()
            .ok_or_else(|| "Nix check catalog is not an array".to_owned())?
            .iter()
            .map(|attribute| {
                attribute
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| "Nix check catalog contains a non-string name".to_owned())
            })
            .collect()
    }

    fn read_derivation_log(&self, root: &Path, store_path: &str) -> Result<Vec<u8>, String> {
        NixCli::read_derivation_log(self, root, store_path)
    }

    fn realize_check(
        &self,
        root: &Path,
        system: &str,
        attribute: &str,
        log_path: &Path,
        cancellation: CancellationToken,
    ) -> NixTaskResult<()> {
        let result = self.run_logged_command(LoggedCommand {
            root,
            args: &build_args(system, attribute),
            log_path,
            cancellation,
            capture_stdout: false,
            failure_code: Some("NixCheckFailed"),
            failure_description: "Nix check failed",
        });
        match result {
            NixTaskResult::Succeeded(_) => NixTaskResult::Succeeded(()),
            NixTaskResult::Failed { code, message } => NixTaskResult::Failed { code, message },
            NixTaskResult::Canceled => NixTaskResult::Canceled,
            NixTaskResult::OperationalError(message) => NixTaskResult::OperationalError(message),
        }
    }
}

pub fn discover_catalog(
    root: &Path,
    systems: &[String],
    backend: &dyn NixBackend,
) -> Result<(Vec<String>, Vec<Notice>), String> {
    let mut ids = vec![
        "static:structure".to_owned(),
        "static:traceability".to_owned(),
    ];
    let mut notices = Vec::new();
    for system in systems {
        let attributes = backend.discover(root, system)?;
        for attribute in attributes {
            if attribute == LEGACY_CHECK_ATTRIBUTE {
                notices.push(Notice {
                    code: "LegacyRecursiveCheckExcluded".to_owned(),
                    message: format!(
                        "excluded legacy recursive check output for {system}: {LEGACY_CHECK_ATTRIBUTE}"
                    ),
                });
                continue;
            }
            ids.push(format!("nix:{system}:{attribute}"));
        }
    }
    ids.sort();
    ids.dedup();
    notices.sort_by(|left, right| left.message.cmp(&right.message));
    notices.dedup_by(|left, right| left.code == right.code && left.message == right.message);
    Ok((ids, notices))
}

pub fn parse_nix_id(id: &str) -> Option<(&str, &str)> {
    let value = id.strip_prefix("nix:")?;
    let (system, attribute) = value.split_once(':')?;
    (!system.is_empty() && !attribute.is_empty()).then_some((system, attribute))
}

pub(super) fn nix_log_store_path(output: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(output);
    let safe = super::details::normalize_display_text(&text);
    let mut expects_log_command = false;
    for line in safe.lines() {
        if line.to_ascii_lowercase().contains("for full logs") {
            if let Some(path) = nix_log_path_from_line(line) {
                return Some(path);
            }
            expects_log_command = true;
            continue;
        }
        if expects_log_command {
            if line.trim().is_empty() {
                continue;
            }
            expects_log_command = false;
            if let Some(path) = nix_log_path_from_line(line) {
                return Some(path);
            }
        }
    }
    None
}

fn nix_log_path_from_line(line: &str) -> Option<String> {
    let (_, command) = line.split_once("nix log")?;
    let command = command.trim_start().trim_start_matches(['\'', '"', '`']);
    let candidate = command
        .split_whitespace()
        .next()?
        .trim_end_matches(['\'', '"', '`', '.', ',', ';', ')', ']']);
    valid_nix_derivation_path(candidate).then(|| candidate.to_owned())
}

fn valid_nix_derivation_path(path: &str) -> bool {
    let Some(name) = path.strip_prefix("/nix/store/") else {
        return false;
    };
    !name.is_empty()
        && name.ends_with(".drv")
        && !name.chars().any(|character| {
            character.is_control() || character.is_whitespace() || character == '/'
        })
}

fn eval_base_args() -> Vec<OsString> {
    ["eval", "--no-write-lock-file", "--no-update-lock-file"]
        .into_iter()
        .map(OsString::from)
        .collect()
}

fn discover_args(system: &str) -> Vec<OsString> {
    let mut args = eval_base_args();
    args.extend([
        OsString::from("--json"),
        OsString::from(format!(".#checks.{system}")),
        OsString::from("--apply"),
        OsString::from("builtins.attrNames"),
    ]);
    args
}

fn build_args(system: &str, attribute: &str) -> Vec<OsString> {
    let installable = format!(
        ".#checks.{system}.{}",
        installable_attribute_path(attribute)
    );
    vec![
        OsString::from("build"),
        OsString::from("--no-link"),
        OsString::from("--no-write-lock-file"),
        OsString::from("--no-update-lock-file"),
        OsString::from(installable),
    ]
}

fn host_nix_system() -> Result<String, String> {
    if let Some(configured) = configured_nix_system() {
        if valid_system_name(&configured) {
            return Ok(configured);
        }
        return Err(format!("invalid configured Nix system '{configured}'"));
    }
    target_nix_system(std::env::consts::ARCH, std::env::consts::OS).ok_or_else(|| {
        format!(
            "unable to infer the host Nix system for {}-{}; select one with --system",
            std::env::consts::ARCH,
            std::env::consts::OS
        )
    })
}

fn configured_nix_system() -> Option<String> {
    let mut configuration = BTreeMap::new();
    for contents in nix_config_contents() {
        for line in contents.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if let Some((name, value)) = line.split_once('=') {
                configuration.insert(name.trim().to_owned(), value.trim().to_owned());
            }
        }
    }
    let eval_system = configuration
        .get("eval-system")
        .filter(|value| !value.is_empty());
    eval_system
        .or_else(|| configuration.get("system"))
        .map(|value| value.trim_matches('"').to_owned())
}

fn nix_config_contents() -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(directory) = std::env::var_os("NIX_CONF_DIR") {
        paths.push(PathBuf::from(directory).join("nix.conf"));
    } else {
        paths.push(PathBuf::from("/etc/nix/nix.conf"));
    }
    if let Some(directory) = std::env::var_os("XDG_CONFIG_HOME") {
        paths.push(PathBuf::from(directory).join("nix/nix.conf"));
    } else if let Some(home) = std::env::var_os("HOME") {
        paths.push(PathBuf::from(home).join(".config/nix/nix.conf"));
    }
    let mut contents = paths
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .collect::<Vec<_>>();
    if let Ok(environment) = std::env::var("NIX_CONFIG") {
        contents.push(environment);
    }
    contents
}

fn target_nix_system(architecture: &str, operating_system: &str) -> Option<String> {
    let platform = match operating_system {
        "linux" => "linux",
        "macos" => "darwin",
        "freebsd" => "freebsd",
        _ => return None,
    };
    let architecture = match (architecture, platform) {
        ("x86_64", "linux") => "x86_64",
        ("aarch64", "linux") => "aarch64",
        ("i686", "linux") => "i686",
        ("arm", "linux") => "armv7l",
        ("riscv64", "linux") | ("riscv64gc", "linux") => "riscv64",
        ("powerpc64", "linux") => "powerpc64",
        ("powerpc64le", "linux") => "powerpc64le",
        ("s390x", "linux") => "s390x",
        ("x86_64", "darwin") => "x86_64",
        ("aarch64", "darwin") => "aarch64",
        ("x86_64", "freebsd") => "x86_64",
        ("aarch64", "freebsd") => "aarch64",
        _ => return None,
    };
    Some(format!("{architecture}-{platform}"))
}

fn installable_attribute_path(value: &str) -> String {
    // Flake fragments are URL-decoded before their quoted attribute path is parsed.
    let mut literal = String::from("\"");
    let mut characters = value.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\\' => literal.push_str("\\\\"),
            '\"' => literal.push_str("\\\""),
            '$' if characters.peek() == Some(&'{') => literal.push_str("\\$"),
            '%' => literal.push_str("%25"),
            '#' => literal.push_str("%23"),
            character if character.is_control() => {
                let mut encoded = [0; 4];
                for byte in character.encode_utf8(&mut encoded).bytes() {
                    literal.push_str(&format!("%{byte:02X}"));
                }
            }
            value => literal.push(value),
        }
    }
    literal.push('\"');
    literal
}

#[cfg(test)]
fn nix_string_literal(value: &str) -> String {
    let mut literal = String::from("\"");
    let mut characters = value.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\\' => literal.push_str("\\\\"),
            '\"' => literal.push_str("\\\""),
            '$' if characters.peek() == Some(&'{') => literal.push_str("\\$"),
            '\n' => literal.push_str("\\n"),
            '\r' => literal.push_str("\\r"),
            '\t' => literal.push_str("\\t"),
            value => literal.push(value),
        }
    }
    literal.push('"');
    literal
}

fn open_private_log(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(file)
}

fn join_stdout(
    reader: Option<thread::JoinHandle<(Vec<u8>, io::Result<usize>)>>,
) -> Option<Vec<u8>> {
    reader.and_then(|reader| reader.join().ok().map(|(bytes, _)| bytes))
}

fn nix_start_error(operation: &str, error: io::Error) -> String {
    if error.kind() == io::ErrorKind::NotFound {
        "Nix execution is selected but the 'nix' CLI is unavailable".to_owned()
    } else {
        format!("unable to start Nix for {operation}: {error}")
    }
}

fn nix_start_error_message(error: io::Error) -> String {
    if error.kind() == io::ErrorKind::NotFound {
        "Nix execution is selected but the 'nix' CLI is unavailable".to_owned()
    } else {
        format!("unable to start Nix task: {error}")
    }
}

fn status_text(status: ExitStatus) -> String {
    status
        .code()
        .map(|code| format!("exit code {code}"))
        .unwrap_or_else(|| "termination by signal".to_owned())
}

fn bounded_process_error(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let safe = super::details::normalize_display_text(&text);
    let first_line = safe
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    super::model::truncate_utf8(first_line.trim(), 512).0
}

#[cfg(test)]
mod tests {
    use super::{
        NixBackend, NixCli, NixTaskResult, build_args, discover_args, discover_catalog,
        host_nix_system, nix_log_store_path, nix_string_literal, parse_nix_id, target_nix_system,
    };
    use crate::check_command::interrupt::{CancellationToken, InterruptFlag};
    use crate::check_command::model::valid_system_name;
    use crate::check_command::test_support::{fixture, nix_is_available};
    use bloomery_test_macros::bloomery;
    use std::ffi::OsString;
    use std::fs;
    use std::path::Path;

    struct CatalogStub {
        names: Vec<String>,
    }

    impl NixBackend for CatalogStub {
        fn host_system(&self, _root: &Path) -> Result<String, String> {
            Ok("x86_64-linux".to_owned())
        }

        fn discover(&self, _root: &Path, _system: &str) -> Result<Vec<String>, String> {
            Ok(self.names.clone())
        }

        fn realize_check(
            &self,
            _root: &Path,
            _system: &str,
            _attribute: &str,
            _log_path: &Path,
            _cancellation: CancellationToken,
        ) -> NixTaskResult<()> {
            unreachable!("catalog test does not realize checks")
        }

        fn read_derivation_log(&self, _root: &Path, _store_path: &str) -> Result<Vec<u8>, String> {
            unreachable!("catalog test does not retrieve derivation logs")
        }
    }

    #[test]
    #[bloomery("CLI-CHECK-NIX-006")]
    #[bloomery("CLI-CHECK-NIX-015")]
    #[bloomery("CLI-CHECK-SELECT-019")]
    fn catalog_discovery_includes_every_attribute_except_legacy_recursive_output() {
        let backend = CatalogStub {
            names: vec![
                "unit-tests".to_owned(),
                "formatter".to_owned(),
                "bloomery:check".to_owned(),
            ],
        };
        let (catalog, notices) =
            discover_catalog(Path::new("."), &["x86_64-linux".to_owned()], &backend)
                .expect("catalog");

        assert!(catalog.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(catalog.contains(&"nix:x86_64-linux:unit-tests".to_owned()));
        assert!(catalog.contains(&"nix:x86_64-linux:formatter".to_owned()));
        assert!(!catalog.contains(&"nix:x86_64-linux:bloomery:check".to_owned()));
        assert_eq!(notices.len(), 1);
        assert!(notices[0].message.contains("bloomery:check"));
    }

    #[test]
    #[bloomery("CLI-CHECK-NIX-009")]
    fn nix_ids_preserve_attribute_names_after_the_system_separator() {
        assert_eq!(
            parse_nix_id("nix:x86_64-linux:checks:with:colons"),
            Some(("x86_64-linux", "checks:with:colons"))
        );
        assert_eq!(parse_nix_id("nix:x86_64-linux:"), None);
    }

    #[test]
    #[bloomery("CLI-CHECK-NIX-011")]
    #[bloomery("CLI-CHECK-NIX-013")]
    #[bloomery("CLI-CHECK-NIX-014")]
    #[bloomery("CLI-CHECK-NIX-016")]
    #[bloomery("CLI-CHECK-NIX-017")]
    fn nix_invocations_are_pure_lock_safe_and_pass_attribute_names_as_literals() {
        let malicious = "bad\"; builtins.abort \"injected ${value}";
        let discovery = strings(&discover_args("x86_64-linux"));
        let build = strings(&build_args("x86_64-linux", malicious));
        for args in [&discovery, &build] {
            assert!(args.iter().any(|arg| arg == "--no-write-lock-file"));
            assert!(args.iter().any(|arg| arg == "--no-update-lock-file"));
            assert!(args.iter().all(|argument| {
                !argument.starts_with("--")
                    || matches!(
                        argument.as_str(),
                        "--no-write-lock-file"
                            | "--no-update-lock-file"
                            | "--json"
                            | "--apply"
                            | "--no-link"
                    )
            }));
            assert!(!args.iter().any(|arg| arg.contains("nix-fast-build")));
        }
        assert!(build.iter().any(|arg| arg == "--no-link"));
        let installable = build.last().expect("quoted Nix installable");
        assert!(installable.contains("x86_64-linux."));
        assert!(!installable.contains(malicious));
        assert!(valid_system_name("aarch64-linux"));
    }

    #[test]
    #[bloomery("CLI-CHECK-NIX-016")]
    fn real_nix_parses_unicode_control_and_quoted_check_attribute_names() {
        if !nix_is_available() {
            eprintln!("skipping real-Nix attribute parser test: Nix store is unavailable");
            return;
        }

        let (root, cache) = fixture(true);
        let system = host_nix_system().expect("host Nix system");
        let attributes = vec![
            "quoted attribute name with spaces".to_owned(),
            "Unicode ☃".to_owned(),
            "line\nbreak".to_owned(),
            "tab\tcharacter".to_owned(),
            format!("control-{}-character", char::from(1)),
            "percent-%0A-and-fragment-#".to_owned(),
        ];
        let names_json = serde_json::to_string(&attributes).expect("JSON attribute names");
        let names_expression = nix_string_literal(&names_json);
        let flake = format!(
            r#"{{
  description = "bloomery unusual attribute fixture";
  outputs = {{ self }}:
    let
      names = builtins.fromJSON {names_expression};
      shared = builtins.derivation {{
        name = "bloomery-unusual-attribute-check";
        system = "{system}";
        builder = "/bin/sh";
        args = [ "-c" "echo passed > $out" ];
      }};
    in {{
      checks."{system}" = builtins.listToAttrs (
        map (name: {{ inherit name; value = shared; }}) names
      );
    }};
}}"#
        );
        fs::write(root.join("flake.nix"), flake).expect("write unusual attribute fixture");

        let backend = NixCli::default();
        let discovered = backend
            .discover(&root, &system)
            .expect("discover unusual Nix attributes");
        let mut sorted_attributes = attributes.clone();
        sorted_attributes.sort();
        assert_eq!(discovered, sorted_attributes);

        for (index, attribute) in attributes.iter().enumerate() {
            let result = backend.realize_check(
                &root,
                &system,
                attribute,
                &root.join(format!("nix-attribute-{index}.log")),
                CancellationToken::new(InterruptFlag::for_test()),
            );
            assert_eq!(
                result,
                NixTaskResult::Succeeded(()),
                "Nix could not build the quoted attribute {attribute:?}"
            );
        }
        assert!(!root.join("flake.lock").exists());

        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(cache);
    }

    #[test]
    #[bloomery("CLI-CHECK-NIX-012")]
    fn a_missing_nix_executable_is_an_operational_error() {
        let missing = NixCli::with_executable("/missing/bloomery-nix-cli");
        let error = missing
            .discover(Path::new("."), "x86_64-linux")
            .expect_err("missing executable");
        assert!(error.contains("nix"));
    }

    #[test]
    #[bloomery("CLI-CHECK-DETAIL-024")]
    fn nix_log_paths_are_parsed_from_failure_hints_and_store_paths_are_validated() {
        let path = "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-failed-check.drv";
        let output = format!("error: check failed\nFor full logs, run 'nix log {path}'.\n");

        assert_eq!(nix_log_store_path(output.as_bytes()).as_deref(), Some(path));
        let multiline = format!("For full logs, run:\n  nix log {path}\n");
        assert_eq!(
            nix_log_store_path(multiline.as_bytes()).as_deref(),
            Some(path)
        );
        assert!(nix_log_store_path(b"For full logs, run: nix log /tmp/not-a-store.drv").is_none());
        assert!(
            nix_log_store_path(format!("build output mentions nix log {path}").as_bytes())
                .is_none()
        );
        assert!(
            nix_log_store_path(format!("build output mentions nix log {path}").as_bytes())
                .is_none()
        );
    }

    #[test]
    fn host_nix_system_uses_platform_mapping() {
        assert_eq!(
            target_nix_system("x86_64", "linux"),
            Some("x86_64-linux".to_owned())
        );
        assert_eq!(
            target_nix_system("aarch64", "macos"),
            Some("aarch64-darwin".to_owned())
        );
        assert!(target_nix_system("mips", "linux").is_none());
        assert!(valid_system_name(&host_nix_system().expect("host system")));
    }

    fn strings(arguments: &[OsString]) -> Vec<String> {
        arguments
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect()
    }
}
