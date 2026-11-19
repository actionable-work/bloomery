use std::{
    fs, io,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
};

use crate::{Result, asset::declarations};

pub(crate) fn build(
    crate_name: &str,
    manifest_dir: &Path,
    index: &Path,
    known: Option<&Path>,
    artifacts: &[PathBuf],
) -> Result<()> {
    for artifact in artifacts {
        let bytes = fs::read(artifact)?;
        for asset in declarations(&bytes) {
            if asset.crate_name != crate_name
                || Path::new(asset.manifest_dir) != manifest_dir
                || asset.is_remote()
            {
                continue;
            }
            let source = asset.resolved_path();
            if !source.is_file() {
                continue;
            }
            let id = asset.id.to_string();
            let entry = index.join(&id);
            let known_entry = known.map(|known| known.join(&id));
            if let Some(known_entry) = known_entry.filter(|path| path.is_dir()) {
                verify_contents(&only_file(&known_entry)?, &source, &id)?;
            } else if entry.is_dir() {
                verify_contents(&only_file(&entry)?, &source, &id)?;
            } else {
                let name = source.file_name().ok_or("asset path has no file name")?;
                fs::create_dir_all(&entry)?;
                fs::copy(&source, entry.join(name))?;
            }
        }
    }
    Ok(())
}

pub(crate) fn merge(index: &Path, sources: &[PathBuf]) -> Result<()> {
    for source in sources.iter().filter(|path| path.is_dir()) {
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let incoming = entry.path().canonicalize()?;
            let id = entry.file_name();
            let target = index.join(&id);
            if target.is_dir() {
                verify_contents(
                    &only_file(&target)?,
                    &only_file(&incoming)?,
                    &id.to_string_lossy(),
                )?;
            } else {
                fs::create_dir_all(index)?;
                symlink(incoming, target)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn file(index: &Path, asset_id: u64) -> Result<Option<PathBuf>> {
    let path = index.join(asset_id.to_string());
    match fs::metadata(&path) {
        Ok(_) => only_file(&path).map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn only_file(entry: &Path) -> Result<PathBuf> {
    let mut files = fs::read_dir(entry)?;
    if let Some(file) = files.next().transpose()?
        && files.next().is_none()
        && file.path().is_file()
    {
        return Ok(file.path());
    }
    Err(format!(
        "asset index entry {} must contain one file",
        entry.display()
    )
    .into())
}

fn verify_contents(existing: &Path, incoming: &Path, id: &str) -> Result<()> {
    if existing.canonicalize()? == incoming.canonicalize()?
        || fs::read(existing)? == fs::read(incoming)?
    {
        return Ok(());
    }
    Err(format!(
        "Topcoat asset ID {id} is published with different contents by {} and {}",
        existing.display(),
        incoming.display()
    )
    .into())
}
