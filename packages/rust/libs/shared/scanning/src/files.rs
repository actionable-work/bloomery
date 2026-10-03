use std::fs;
use std::path::{Path, PathBuf};

pub fn expand_globs(root: &Path, patterns: &[String]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    walk_files(root, root, patterns, &mut files)?;
    files.sort();
    files.dedup();
    Ok(files)
}

fn walk_files(
    root: &Path,
    directory: &Path,
    patterns: &[String],
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("unable to read {}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("unable to read directory entry: {error}"))?;
        let path = entry.path();
        let name = entry.file_name();
        if matches!(
            name.to_str(),
            Some(".git" | "target" | ".direnv" | "result" | "node_modules")
        ) {
            continue;
        }
        if path.is_dir() {
            walk_files(root, &path, patterns, files)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| format!("path escaped root: {}", path.display()))?;
            if patterns
                .iter()
                .any(|pattern| glob_matches(pattern, relative))
            {
                files.push(path);
            }
        }
    }
    Ok(())
}

fn glob_matches(pattern: &str, path: &Path) -> bool {
    let pattern_parts: Vec<&str> = pattern.trim_matches('/').split('/').collect();
    let path_string = path
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    let path_parts: Vec<&str> = path_string.trim_matches('/').split('/').collect();
    match_parts(&pattern_parts, &path_parts)
}

fn match_parts(pattern: &[&str], path: &[&str]) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }
    if pattern[0] == "**" {
        return match_parts(&pattern[1..], path)
            || (!path.is_empty() && match_parts(pattern, &path[1..]));
    }
    !path.is_empty()
        && segment_matches(pattern[0], path[0])
        && match_parts(&pattern[1..], &path[1..])
}

fn segment_matches(pattern: &str, value: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let value: Vec<char> = value.chars().collect();
    let mut states = vec![false; value.len() + 1];
    states[0] = true;
    for character in pattern {
        let mut next = vec![false; value.len() + 1];
        match character {
            '*' => {
                let mut reachable = false;
                for index in 0..=value.len() {
                    reachable |= states[index];
                    next[index] = reachable;
                }
            }
            '?' => {
                next[1..].copy_from_slice(&states[..value.len()]);
            }
            literal => {
                for index in 0..value.len() {
                    if states[index] && value[index] == literal {
                        next[index + 1] = true;
                    }
                }
            }
        }
        states = next;
    }
    states[value.len()]
}
