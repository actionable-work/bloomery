use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use topcoat::asset::{Asset, AssetBundle, Manifest, ManifestEntry, asset};

pub const BLOOMERY_CSS: Asset = asset!("../assets/bloomery.css");
pub const BLOOMERY_LOGO: Asset = asset!("../assets/bloomery-forge.svg");

pub const CSS_CONTENT: &str = include_str!("../assets/bloomery.css");
pub const LOGO_CONTENT: &str = include_str!("../assets/bloomery-forge.svg");

/// Prepare an on-disk asset bundle and manifest for Topcoat router serving.
#[allow(clippy::collapsible_if)]
pub fn prepare_asset_bundle(dir: impl AsRef<Path>) -> io::Result<AssetBundle> {
    let dir = dir.as_ref();
    if dir.join("manifest.toml").is_file() {
        if let Ok(bundle) = AssetBundle::load_dir(dir) {
            if bundle.get(BLOOMERY_CSS.id()).is_some() && bundle.get(BLOOMERY_LOGO.id()).is_some() {
                return Ok(bundle);
            }
        }
    }

    fs::create_dir_all(dir)?;

    let css_file = "bloomery.css";
    let logo_file = "bloomery-forge.svg";

    fs::write(dir.join(css_file), CSS_CONTENT)?;
    fs::write(dir.join(logo_file), LOGO_CONTENT)?;

    let manifest = Manifest {
        version: 1,
        assets: vec![
            ManifestEntry {
                id: BLOOMERY_CSS.id(),
                file: css_file.to_string(),
                hash: simple_hash(CSS_CONTENT.as_bytes()),
                content_type: "text/css".to_string(),
            },
            ManifestEntry {
                id: BLOOMERY_LOGO.id(),
                file: logo_file.to_string(),
                hash: simple_hash(LOGO_CONTENT.as_bytes()),
                content_type: "image/svg+xml".to_string(),
            },
        ],
    };

    manifest.save(dir.join("manifest.toml"))?;

    AssetBundle::load_dir(dir)
}

/// Fallback location for the asset bundle directory.
#[allow(clippy::collapsible_if)]
pub fn default_bundle_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("BLOOMERY_ASSET_DIR") {
        PathBuf::from(dir)
    } else if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("assets");
            if candidate.join("manifest.toml").is_file() {
                if let Ok(bundle) = AssetBundle::load_dir(&candidate) {
                    if bundle.get(BLOOMERY_CSS.id()).is_some()
                        && bundle.get(BLOOMERY_LOGO.id()).is_some()
                    {
                        return candidate;
                    }
                }
            }
        }
        let id_hash = format!(
            "{:016x}-{:016x}",
            BLOOMERY_CSS.id().as_u64(),
            BLOOMERY_LOGO.id().as_u64()
        );
        std::env::temp_dir().join(format!("bloomery-assets-{id_hash}"))
    } else {
        let id_hash = format!(
            "{:016x}-{:016x}",
            BLOOMERY_CSS.id().as_u64(),
            BLOOMERY_LOGO.id().as_u64()
        );
        std::env::temp_dir().join(format!("bloomery-assets-{id_hash}"))
    }
}

fn simple_hash(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}
