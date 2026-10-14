use serde::Deserialize;
use std::fs;
use std::path::Path;

/// The repository documentation coverage manifest.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Manifest {
    /// Repository-relative globs that enumerate authored documentation.
    #[serde(default)]
    pub roots: Vec<String>,
    /// One entry per authored documentation surface.
    #[serde(default)]
    pub surfaces: Vec<Surface>,
}

/// One registered documentation surface.
#[derive(Debug, Clone, Deserialize)]
pub struct Surface {
    /// Repository-relative path of the surface.
    pub path: String,
    /// Specification features (`AREA/FEATURE`) the surface documents.
    #[serde(default)]
    pub features: Vec<String>,
}

/// Repository-relative path of the documentation coverage manifest.
pub const MANIFEST_RELATIVE_PATH: &str = ".bloomery/docs.manifest.toml";

/// Load the documentation coverage manifest.
pub fn load_manifest(root: &Path) -> Result<Manifest, String> {
    let path = root.join(MANIFEST_RELATIVE_PATH);
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("unable to read {}: {error}", path.display()))?;
    toml::from_str(&contents)
        .map_err(|error| format!("unable to parse {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempRoot;

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-CONSISTENCY-COVERAGE-001"))]
    #[cfg_attr(any(), bloomery("DOCS-CONSISTENCY-COVERAGE-002"))]
    fn loads_roots_and_surfaces() {
        let fixture = TempRoot::new("manifest");
        fixture.write(
            MANIFEST_RELATIVE_PATH,
            "roots = [\"README.md\", \"docs/*.md\"]\n\n\
             [[surfaces]]\npath = \"README.md\"\nfeatures = [\"CLI/INTERFACE\"]\n\n\
             [[surfaces]]\npath = \"docs/a.md\"\nfeatures = [\"DOCS/CONTENT\", \"DOCS/UI\"]\n",
        );

        let manifest = load_manifest(fixture.path()).expect("manifest should load");
        assert_eq!(manifest.roots, vec!["README.md", "docs/*.md"]);
        assert_eq!(manifest.surfaces.len(), 2);
        assert_eq!(manifest.surfaces[0].path, "README.md");
        assert_eq!(manifest.surfaces[0].features, vec!["CLI/INTERFACE"]);
        assert_eq!(manifest.surfaces[1].features.len(), 2);
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-CONSISTENCY-COVERAGE-007"))]
    fn manifest_is_read_from_bloomery_directory() {
        let fixture = TempRoot::new("manifest-location");
        fixture.write(".bloomery/docs.manifest.toml", "roots = [\"README.md\"]\n");

        let manifest = load_manifest(fixture.path()).expect("manifest should load");
        assert_eq!(manifest.roots, vec!["README.md"]);

        let legacy = TempRoot::new("manifest-legacy-root");
        legacy.write("docs.manifest.toml", "roots = [\"README.md\"]\n");
        assert!(
            load_manifest(legacy.path()).is_err(),
            "a root-level manifest must not be read"
        );
    }
}
