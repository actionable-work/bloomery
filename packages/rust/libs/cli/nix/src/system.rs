//! Nix system names: validation and host system detection.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// Whether `value` is a safe Nix system name for attribute paths and arguments.
/// Requested systems with repeats removed, keeping first-occurrence order.
pub fn deduplicate_systems(systems: &[String]) -> Vec<String> {
    let mut deduplicated = Vec::new();
    for system in systems {
        if !deduplicated.contains(system) {
            deduplicated.push(system.clone());
        }
    }
    deduplicated
}

pub fn valid_system_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

/// Resolve the host Nix system without evaluating Nix.
///
/// A configured `eval-system` or `system` setting from the Nix configuration
/// wins; otherwise the Rust target platform is mapped to its Nix system name.
pub fn host_nix_system() -> Result<String, String> {
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
        .filter_map(|path| std::fs::read_to_string(path).ok())
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

#[cfg(test)]
mod tests {
    use super::{host_nix_system, target_nix_system, valid_system_name};

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

    #[test]
    fn system_names_reject_path_syntax() {
        assert!(valid_system_name("x86_64-linux"));
        assert!(!valid_system_name("../x86_64-linux"));
        assert!(!valid_system_name(""));
    }
}
