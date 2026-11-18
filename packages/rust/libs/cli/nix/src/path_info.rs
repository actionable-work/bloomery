//! `nix path-info --json` responses.
//!
//! Current Nix reports an object keyed by store path whose value is `null` for
//! an invalid (unrealized) path. Older Nix reports an array of entries carrying
//! `path` and, for invalid paths, `"valid": false`. Both shapes parse to the
//! same [`PathInfo`].

use serde_json::Value;

/// One store path reported by `nix path-info`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathEntry {
    pub path: String,
    /// Uncompressed NAR size, when Nix reported one for a valid path.
    pub nar_size: Option<u64>,
}

/// Parsed `nix path-info --json` response.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathInfo {
    /// Paths present in the store, in response order.
    pub valid: Vec<PathEntry>,
    /// Paths Nix reported as not present in the store.
    pub invalid: Vec<String>,
}

impl PathInfo {
    /// Whether `path` is reported valid.
    pub fn is_valid(&self, path: &str) -> bool {
        self.valid.iter().any(|entry| entry.path == path)
    }

    /// Whether `path` is reported at all, valid or not.
    pub fn reports(&self, path: &str) -> bool {
        self.is_valid(path) || self.invalid.iter().any(|invalid| invalid == path)
    }
}

/// Parse the stdout of `nix path-info --json`.
pub fn parse_path_info(bytes: &[u8]) -> Result<PathInfo, String> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("Nix returned invalid path-info JSON: {error}"))?;
    let mut info = PathInfo::default();
    match value {
        Value::Object(map) => {
            for (path, entry) in map {
                if entry.is_null() {
                    info.invalid.push(path);
                } else {
                    info.valid.push(PathEntry {
                        nar_size: nar_size(&entry),
                        path,
                    });
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                let path = item
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "Nix path-info entry has no path".to_owned())?
                    .to_owned();
                if item.get("valid").and_then(Value::as_bool) == Some(false) {
                    info.invalid.push(path);
                } else {
                    info.valid.push(PathEntry {
                        nar_size: nar_size(&item),
                        path,
                    });
                }
            }
        }
        _ => return Err("Nix path-info is neither an object nor an array".to_owned()),
    }
    Ok(info)
}

fn nar_size(entry: &Value) -> Option<u64> {
    entry.get("narSize").and_then(Value::as_u64)
}

#[cfg(test)]
mod tests {
    use super::{PathEntry, parse_path_info};

    #[test]
    fn object_responses_separate_valid_and_null_paths() {
        let info = parse_path_info(
            br#"{"/nix/store/a": {"narSize": 10}, "/nix/store/b": null, "/nix/store/c": {}}"#,
        )
        .expect("object");
        assert_eq!(
            info.valid,
            vec![
                PathEntry {
                    path: "/nix/store/a".to_owned(),
                    nar_size: Some(10),
                },
                PathEntry {
                    path: "/nix/store/c".to_owned(),
                    nar_size: None,
                },
            ]
        );
        assert_eq!(info.invalid, vec!["/nix/store/b".to_owned()]);
        assert!(info.is_valid("/nix/store/a"));
        assert!(!info.is_valid("/nix/store/b"));
        assert!(info.reports("/nix/store/b"));
        assert!(!info.reports("/nix/store/d"));
    }

    #[test]
    fn array_responses_honor_the_valid_flag() {
        let info = parse_path_info(
            br#"[{"path": "/nix/store/a", "narSize": 3}, {"path": "/nix/store/b", "valid": false}]"#,
        )
        .expect("array");
        assert!(info.is_valid("/nix/store/a"));
        assert_eq!(info.valid[0].nar_size, Some(3));
        assert_eq!(info.invalid, vec!["/nix/store/b".to_owned()]);
    }

    #[test]
    fn malformed_responses_are_rejected() {
        assert!(parse_path_info(b"not json").is_err());
        assert!(parse_path_info(b"42").is_err());
        assert!(parse_path_info(br#"[{"narSize": 1}]"#).is_err());
    }
}
