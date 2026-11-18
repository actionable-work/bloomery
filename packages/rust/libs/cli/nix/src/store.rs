//! Store paths, derivation paths, and `nix` invocation details.

use std::ffi::OsString;

const STORE_PREFIX: &str = "/nix/store/";

/// Whether `value` is an absolute path inside the Nix store.
pub fn is_store_path(value: &str) -> bool {
    value.starts_with(STORE_PREFIX)
}

/// Prefix a store-relative name with the store directory when it lacks one.
pub fn full_store_path(value: &str) -> String {
    if is_store_path(value) {
        value.to_owned()
    } else {
        format!("{STORE_PREFIX}{value}")
    }
}

/// Whether `path` is one absolute `.drv` store path with no unsafe characters.
pub fn valid_derivation_path(path: &str) -> bool {
    let Some(name) = path.strip_prefix(STORE_PREFIX) else {
        return false;
    };
    !name.is_empty()
        && name.ends_with(".drv")
        && !name.chars().any(|character| {
            character.is_control() || character.is_whitespace() || character == '/'
        })
}

/// Extract the derivation named by a `nix build` "Cannot build '…'" line.
pub fn parse_cannot_build(line: &str) -> Option<String> {
    let start = line.find("Cannot build '")? + "Cannot build '".len();
    let rest = &line[start..];
    let end = rest.find('\'')?;
    let derivation = &rest[..end];
    valid_derivation_path(derivation).then(|| derivation.to_owned())
}

/// Read one `{ success; drvPath; }` entry produced by a `tryEval drvPath`
/// projection: whether evaluation succeeded, and the derivation path when it
/// is a valid one.
pub fn evaluated_derivation(entry: &serde_json::Value) -> (bool, Option<String>) {
    let success = entry
        .get("success")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let derivation = entry
        .get("drvPath")
        .and_then(serde_json::Value::as_str)
        .filter(|path| valid_derivation_path(path))
        .map(str::to_owned);
    (success, derivation)
}

/// `nix eval` arguments that never write or update the flake lock file.
pub fn eval_base_args() -> Vec<OsString> {
    ["eval", "--no-write-lock-file", "--no-update-lock-file"]
        .into_iter()
        .map(OsString::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{full_store_path, parse_cannot_build, valid_derivation_path};

    #[test]
    fn store_paths_are_absolute_and_derivations_are_validated() {
        assert_eq!(full_store_path("abc-x.drv"), "/nix/store/abc-x.drv");
        assert_eq!(full_store_path("/nix/store/abc-x"), "/nix/store/abc-x");
        assert!(valid_derivation_path("/nix/store/abc-x.drv"));
        assert!(!valid_derivation_path("/nix/store/abc-x"));
        assert!(!valid_derivation_path("/nix/store/a b.drv"));
        assert!(!valid_derivation_path("/tmp/abc-x.drv"));
        assert_eq!(
            parse_cannot_build("error: Cannot build '/nix/store/abc-x.drv'."),
            Some("/nix/store/abc-x.drv".to_owned())
        );
        assert_eq!(parse_cannot_build("error: Cannot build 'nope'."), None);
    }
}
