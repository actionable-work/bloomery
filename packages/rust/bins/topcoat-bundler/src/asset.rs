use std::path::{Component, Path, PathBuf};

const PREFIX: &[u8] = b"TOPCOAT_ASSET";
const ENCODED_SIZE: usize = 2048;

pub(crate) struct Declaration<'a> {
    pub id: u64,
    pub path: &'a str,
    pub crate_name: &'a str,
    pub manifest_dir: &'a str,
    pub source_file: &'a str,
    pub rename: Option<&'a str>,
    pub extension: Option<&'a str>,
    pub checksum: Option<&'a str>,
    pub content_type: Option<&'a str>,
}

pub(crate) fn declarations(bytes: &[u8]) -> impl Iterator<Item = Declaration<'_>> {
    bytes
        .windows(ENCODED_SIZE)
        .filter_map(|record| decode(record.strip_prefix(PREFIX)?))
        .filter(|asset| !asset.path.is_empty())
}

fn decode(mut data: &[u8]) -> Option<Declaration<'_>> {
    Some(Declaration {
        id: u64::from_le_bytes(chunk(&mut data)?),
        path: string(&mut data)?,
        crate_name: string(&mut data)?,
        manifest_dir: string(&mut data)?,
        source_file: string(&mut data)?,
        rename: optional_string(&mut data)?,
        extension: optional_string(&mut data)?,
        checksum: optional_string(&mut data)?,
        content_type: optional_string(&mut data)?,
    })
}

fn string<'a>(data: &mut &'a [u8]) -> Option<&'a str> {
    let len = u16::from_le_bytes(chunk(data)?);
    str::from_utf8(data.split_off(..usize::from(len))?).ok()
}

fn chunk<const N: usize>(data: &mut &[u8]) -> Option<[u8; N]> {
    let (value, rest) = data.split_first_chunk()?;
    *data = rest;
    Some(*value)
}

fn optional_string<'a>(data: &mut &'a [u8]) -> Option<Option<&'a str>> {
    match data.split_off_first()? {
        0 => Some(None),
        1 => Some(Some(string(data)?)),
        _ => None,
    }
}

impl Declaration<'_> {
    pub(crate) fn is_remote(&self) -> bool {
        self.path.starts_with("http://") || self.path.starts_with("https://")
    }

    pub(crate) fn resolved_path(&self) -> PathBuf {
        let path = Path::new(self.path);
        let manifest = Path::new(self.manifest_dir);
        if path.is_absolute() {
            return normalize(path);
        }
        let source;
        let anchor = match path.components().next() {
            Some(Component::CurDir | Component::ParentDir) => {
                source = self.source_path(manifest);
                source.parent().unwrap_or(manifest)
            }
            _ => manifest,
        };
        normalize(&anchor.join(path))
    }

    fn source_path(&self, manifest: &Path) -> PathBuf {
        let source = Path::new(self.source_file);
        if source.is_absolute() {
            return source.to_owned();
        }
        manifest
            .ancestors()
            .map(|dir| dir.join(source))
            .find(|path| path.exists())
            .unwrap_or_else(|| manifest.join(source))
    }

    pub(crate) fn description(&self) -> String {
        format!(
            "Topcoat asset \"{}\" declared in crate `{}` ({})",
            self.path, self.crate_name, self.source_file
        )
    }
}

fn normalize(path: &Path) -> PathBuf {
    let mut output = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !output.pop() {
                    output.push(component);
                }
            }
            _ => output.push(component),
        }
    }
    output
}
