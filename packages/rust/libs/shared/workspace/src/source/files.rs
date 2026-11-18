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
