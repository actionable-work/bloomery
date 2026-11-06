use std::fs;
use std::path::Path;

pub(super) fn filesystem_entries(root: &Path) -> Result<(Vec<String>, Vec<String>), String> {
    let mut files = Vec::new();
    let mut directories = Vec::new();
    walk_files(root, root, &mut files, &mut directories)?;
    files.sort();
    directories.sort();
    Ok((files, directories))
}

fn walk_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<String>,
    directories: &mut Vec<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("unable to read {}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("unable to read directory entry: {error}"))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if entry.file_name().to_str().is_some_and(ignored_name) || file_type.is_symlink() {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .ok()
            .and_then(Path::to_str)
            .ok_or_else(|| "source path is not repository-relative UTF-8".to_owned())?
            .replace(std::path::MAIN_SEPARATOR, "/");
        if file_type.is_dir() {
            directories.push(relative);
            walk_files(root, &path, files, directories)?;
        } else if file_type.is_file() {
            files.push(relative);
        }
    }
    Ok(())
}

pub(crate) fn ignored_name(name: &str) -> bool {
    matches!(
        name,
        ".git" | ".jj" | "target" | ".direnv" | "result" | "node_modules"
    )
}

pub(crate) fn glob_matches(pattern: &str, path: &Path) -> bool {
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
