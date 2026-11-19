use std::{
    collections::{BTreeMap, HashSet, btree_map::Entry},
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    Result,
    asset::{Declaration, declarations},
    index,
};

pub(crate) const MANIFEST_NAME: &str = "manifest.toml";

#[derive(Deserialize, Serialize)]
pub(crate) struct Manifest {
    pub version: u32,
    pub assets: Vec<ManifestEntry>,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct ManifestEntry {
    pub id: u64,
    pub file: String,
    pub hash: String,
    pub content_type: String,
}

pub(crate) fn write<'a>(
    index: &Path,
    artifact: &Path,
    outputs: impl IntoIterator<Item = &'a PathBuf>,
) -> Result<()> {
    let bytes = fs::read(artifact)?;
    let mut seen = HashSet::new();
    let mut manifest = Manifest {
        version: 1,
        assets: Vec::new(),
    };
    let mut files = BTreeMap::new();
    let mut unresolved = Vec::new();
    for asset in declarations(&bytes) {
        if asset.is_remote() {
            unresolved.push(format!(
                "{} is a remote URL; vendor the file into the crate",
                asset.description()
            ));
            continue;
        }
        if !seen.insert(asset.id) {
            continue;
        }
        let Some(source) = index::file(index, asset.id)? else {
            unresolved.push(format!(
                "{} was not found: no file at {} when the crate was built",
                asset.description(),
                asset.resolved_path().display()
            ));
            continue;
        };
        let content = fs::read(source)?;
        let hash = format!("{:x}", Sha256::digest(&content));
        verify_checksum(&asset, &hash)?;
        let file = file_name(&asset, &hash[..16])?;
        let content_type = asset
            .content_type
            .unwrap_or_else(|| guess_content_type(&file));
        match files.entry(file.clone()) {
            Entry::Vacant(entry) => {
                entry.insert((content_type, content));
            }
            Entry::Occupied(entry) => {
                let (previous_type, previous_content) = entry.get();
                if *previous_type != content_type {
                    return Err(format!(
                        "bundled file {file} is declared with content types {previous_type} and {content_type}"
                    ).into());
                }
                if *previous_content != content {
                    return Err(
                        format!("bundled file {file} is declared with different contents").into(),
                    );
                }
            }
        }
        manifest.assets.push(ManifestEntry {
            id: asset.id,
            file,
            hash,
            content_type: content_type.to_owned(),
        });
    }
    if !unresolved.is_empty() {
        return Err(unresolved.join("\n").into());
    }
    if manifest.assets.is_empty() {
        return Ok(());
    }
    let manifest = toml::to_string(&manifest)?;
    for output in outputs {
        fs::create_dir_all(output)?;
        for (file, (_, content)) in &files {
            replace_file(&output.join(file), content)?;
        }
        replace_file(&output.join(MANIFEST_NAME), manifest.as_bytes())?;
    }
    Ok(())
}

fn replace_file(path: &Path, content: &[u8]) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    fs::write(path, content)?;
    Ok(())
}

fn verify_checksum(asset: &Declaration<'_>, hash: &str) -> Result<()> {
    if let Some(checksum) = asset.checksum {
        let expected = checksum.strip_prefix("sha256:").ok_or_else(|| {
            format!(
                "{} uses unsupported checksum {checksum}",
                asset.description()
            )
        })?;
        if expected != hash {
            return Err(format!(
                "{} failed checksum verification: expected {checksum}, found sha256:{hash}",
                asset.description()
            )
            .into());
        }
    }
    Ok(())
}

fn file_name(asset: &Declaration<'_>, short_hash: &str) -> Result<String> {
    let path = asset.resolved_path();
    let stem = asset
        .rename
        .or_else(|| path.file_stem()?.to_str())
        .unwrap_or("asset");
    let extension = asset
        .extension
        .or_else(|| path.extension()?.to_str())
        .unwrap_or("");
    if [stem, extension]
        .iter()
        .any(|part| part.contains(['/', '\\', '\0']))
    {
        return Err(format!(
            "{} uses a rename or extension that is not a file name",
            asset.description()
        )
        .into());
    }
    Ok(match (stem, extension) {
        ("", "") => short_hash.to_owned(),
        ("", extension) => format!("{short_hash}.{extension}"),
        (stem, "") => format!("{stem}.{short_hash}"),
        (stem, extension) => format!("{stem}-{short_hash}.{extension}"),
    })
}

fn guess_content_type(file: &str) -> &'static str {
    match Path::new(file)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "avif" => "image/avif",
        "css" => "text/css",
        "gif" => "image/gif",
        "htm" | "html" => "text/html",
        "ico" => "image/x-icon",
        "jpg" | "jpeg" => "image/jpeg",
        "js" | "mjs" => "text/javascript",
        "json" => "application/json",
        "map" | "txt" => "text/plain",
        "mp4" => "video/mp4",
        "otf" => "application/font-sfnt",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        "webm" => "video/webm",
        "webp" => "image/webp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "xml" => "text/xml",
        _ => "application/octet-stream",
    }
}
