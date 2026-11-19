use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{
    bundle::{self, MANIFEST_NAME, Manifest},
    index,
};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "bloomery-topcoat-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        Self(dir)
    }

    fn file(&self, path: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }

    fn binary(&self, records: &[(u64, &str, [Option<&str>; 4])]) -> PathBuf {
        let mut bytes = b"ELF TOPCOAT_ASSET marker without a body".to_vec();
        for (id, path, options) in records {
            let start = bytes.len();
            bytes.extend_from_slice(b"TOPCOAT_ASSET");
            bytes.extend_from_slice(&id.to_le_bytes());
            for value in [*path, "example", self.0.to_str().unwrap(), "src/lib.rs"] {
                append_string(&mut bytes, value);
            }
            for option in options {
                bytes.push(u8::from(option.is_some()));
                if let Some(value) = option {
                    append_string(&mut bytes, value);
                }
            }
            bytes.resize(start + 2048, 0);
        }
        self.file("binary", bytes)
    }

    fn build_index(&self, binary: &Path) -> PathBuf {
        let index = self.0.join("index");
        index::build("example", &self.0, &index, None, &[binary.to_owned()]).unwrap();
        index
    }

    fn bundle(&self, binary: &Path, index: &Path) -> crate::Result<PathBuf> {
        let out = self.0.join("out");
        bundle::write(index, binary, [&out])?;
        Ok(out)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn append_string(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&u16::try_from(value.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

#[test]
#[cfg_attr(any(), bloomery("NIXLIB-GRAPH-ASSETS-001"))]
#[cfg_attr(any(), bloomery("NIXLIB-GRAPH-ASSETS-004"))]
#[cfg_attr(any(), bloomery("NIXLIB-GRAPH-ASSETS-015"))]
fn bundle_uses_asset_ids_and_preserves_declaration_order() {
    let fixture = Fixture::new();
    fixture.file("a/site.css", "a");
    fixture.file("src/b/site.css", "b");
    let binary = fixture.binary(&[
        (u64::MAX, "a/site.css", [None; 4]),
        (3, "./b/site.css", [None; 4]),
        (u64::MAX, "a/site.css", [None; 4]),
    ]);
    let index = fixture.build_index(&binary);
    let out = fixture.bundle(&binary, &index).unwrap();
    let manifest: Manifest =
        toml::from_str(&fs::read_to_string(out.join(MANIFEST_NAME)).unwrap()).unwrap();
    assert_eq!(manifest.version, 1);
    assert_eq!(
        manifest
            .assets
            .iter()
            .map(|asset| asset.id)
            .collect::<Vec<_>>(),
        [u64::MAX, 3]
    );
    assert_eq!(
        manifest.assets[0].hash,
        "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb"
    );
    for (entry, expected) in manifest.assets.iter().zip([b"a", b"b"]) {
        assert_eq!(entry.file, format!("site-{}.css", &entry.hash[..16]));
        assert_eq!(entry.content_type, "text/css");
        assert_eq!(fs::read(out.join(&entry.file)).unwrap(), expected);
    }
}

#[test]
#[cfg_attr(any(), bloomery("NIXLIB-GRAPH-ASSETS-005"))]
#[cfg_attr(any(), bloomery("NIXLIB-GRAPH-ASSETS-007"))]
fn missing_and_remote_assets_are_reported() {
    let fixture = Fixture::new();
    let binary = fixture.binary(&[
        (1, "assets/missing.css", [None; 4]),
        (2, "https://example.test/font.woff2", [None; 4]),
    ]);
    let index = fixture.build_index(&binary);
    let message = fixture.bundle(&binary, &index).unwrap_err().to_string();
    for text in [
        "assets/missing.css",
        "`example`",
        "src/lib.rs",
        "https://example.test/font.woff2",
        "remote URL",
    ] {
        assert!(message.contains(text), "{message}");
    }
    assert!(!fixture.0.join("out").exists());
}

#[test]
#[cfg_attr(any(), bloomery("NIXLIB-GRAPH-ASSETS-008"))]
#[cfg_attr(any(), bloomery("NIXLIB-GRAPH-ASSETS-009"))]
fn naming_and_type_options_are_preserved() {
    let fixture = Fixture::new();
    for name in [
        "source.txt",
        "hash.txt",
        "LICENSE",
        "script.js",
        "unknown.other",
    ] {
        fixture.file(name, "a");
    }
    let binary = fixture.binary(&[
        (
            1,
            "source.txt",
            [Some("styles"), Some("css"), None, Some("text/x-custom")],
        ),
        (2, "hash.txt", [Some(""), None, None, None]),
        (3, "LICENSE", [None; 4]),
        (4, "script.js", [None; 4]),
        (5, "unknown.other", [None, Some(""), None, None]),
    ]);
    let index = fixture.build_index(&binary);
    let out = fixture.bundle(&binary, &index).unwrap();
    let manifest: Manifest =
        toml::from_str(&fs::read_to_string(out.join(MANIFEST_NAME)).unwrap()).unwrap();
    assert_eq!(
        manifest
            .assets
            .iter()
            .map(|asset| (asset.file.as_str(), asset.content_type.as_str()))
            .collect::<Vec<_>>(),
        [
            ("styles-ca978112ca1bbdca.css", "text/x-custom"),
            ("ca978112ca1bbdca.txt", "text/plain"),
            ("LICENSE.ca978112ca1bbdca", "application/octet-stream"),
            ("script-ca978112ca1bbdca.js", "text/javascript"),
            ("unknown.ca978112ca1bbdca", "application/octet-stream"),
        ]
    );
}

#[test]
#[cfg_attr(any(), bloomery("NIXLIB-GRAPH-ASSETS-010"))]
fn checksums_must_match() {
    let fixture = Fixture::new();
    fixture.file("a.txt", "a");
    for (checksum, expected) in [
        (
            "sha256:ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb",
            None,
        ),
        ("sha256:wrong", Some("failed checksum verification")),
        ("md5:00", Some("unsupported checksum")),
    ] {
        let binary = fixture.binary(&[(1, "a.txt", [None, None, Some(checksum), None])]);
        let result = fixture.bundle(&binary, &fixture.build_index(&binary));
        if let Some(error) = expected {
            let message = result.unwrap_err().to_string();
            assert!(
                message.contains(error) && message.contains("a.txt"),
                "{message}"
            );
        } else {
            result.unwrap();
        }
    }
}

#[test]
#[cfg_attr(any(), bloomery("NIXLIB-GRAPH-ASSETS-018"))]
fn conflicting_content_types_fail() {
    let fixture = Fixture::new();
    fixture.file("logo.svg", "<svg/>");
    let binary = fixture.binary(&[
        (1, "logo.svg", [None, None, None, Some("image/svg+xml")]),
        (2, "logo.svg", [None, None, None, Some("text/plain")]),
    ]);
    let error = fixture
        .bundle(&binary, &fixture.build_index(&binary))
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("logo-") && error.contains("image/svg+xml and text/plain"),
        "{error}"
    );
}

#[test]
#[cfg_attr(any(), bloomery("NIXLIB-GRAPH-ASSETS-019"))]
fn indexing_rejects_conflicting_bytes_for_the_same_id() {
    let fixture = Fixture::new();
    fixture.file("a.css", "a");
    fixture.file("b.css", "b");
    let binary = fixture.binary(&[(7, "a.css", [None; 4]), (7, "b.css", [None; 4])]);
    let index = fixture.0.join("index");
    let error = index::build("example", &fixture.0, &index, None, &[binary]).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("asset ID 7 is published with different contents")
    );
    let binary = fixture.binary(&[(7, "b.css", [None; 4])]);
    let other = fixture.0.join("other");
    let error = index::build(
        "example",
        &fixture.0,
        &other,
        Some(&index),
        std::slice::from_ref(&binary),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("asset ID 7 is published with different contents")
    );
    index::build("example", &fixture.0, &other, None, &[binary]).unwrap();
    let error = index::merge(&index, &[other]).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("asset ID 7 is published with different contents")
    );
}

#[test]
#[cfg_attr(any(), bloomery("NIXLIB-GRAPH-ASSETS-020"))]
fn conflicting_file_contents_fail() {
    let fixture = Fixture::new();
    fixture.file("a.txt", "a");
    fixture.file("b.txt", "b");
    let binary = fixture.binary(&[
        (1, "a.txt", [Some(""), Some("3e23e8160039594a"), None, None]),
        (2, "b.txt", [Some("ca978112ca1bbdca"), Some(""), None, None]),
    ]);
    let error = fixture
        .bundle(&binary, &fixture.build_index(&binary))
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("is declared with different contents"),
        "{error}"
    );
}

#[test]
#[cfg_attr(any(), bloomery("NIXLIB-GRAPH-ASSETS-021"))]
fn path_separators_in_names_fail() {
    let fixture = Fixture::new();
    fixture.file("a.txt", "a");
    let binary = fixture.binary(&[(1, "a.txt", [Some("../a"), None, None, None])]);
    let error = fixture
        .bundle(&binary, &fixture.build_index(&binary))
        .unwrap_err()
        .to_string();
    assert!(error.contains("rename or extension"));
    assert!(!fixture.0.join("out").exists());
}

#[test]
#[cfg_attr(any(), bloomery("NIXLIB-GRAPH-ASSETS-022"))]
fn binaries_without_declarations_are_untouched() {
    let fixture = Fixture::new();
    let binary = fixture.binary(&[]);
    let index = fixture.build_index(&binary);
    let output = fixture.bundle(&binary, &index).unwrap();
    assert!(!output.exists());
}

#[test]
#[cfg_attr(any(), bloomery("NIXLIB-GRAPH-ASSETS-024"))]
fn indexing_distinguishes_two_builds_with_the_same_crate_name() {
    let dependency = Fixture::new();
    dependency.file("logo.svg", "dependency");
    let dependency_binary = dependency.binary(&[(1, "logo.svg", [None; 4])]);
    let own = Fixture::new();
    own.file("logo.svg", "own");
    let own_binary = own.binary(&[(2, "logo.svg", [None; 4])]);
    let executable = own.file(
        "linked",
        [
            fs::read(dependency_binary).unwrap(),
            fs::read(own_binary).unwrap(),
        ]
        .concat(),
    );
    let index = own.build_index(&executable);
    assert!(index::file(&index, 1).unwrap().is_none());
    assert_eq!(
        fs::read(index::file(&index, 2).unwrap().unwrap()).unwrap(),
        b"own"
    );
}
