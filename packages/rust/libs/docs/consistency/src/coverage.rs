use crate::glob;
use crate::manifest::{MANIFEST_RELATIVE_PATH, Manifest};
use bloomery_model::Diagnostic;
use std::collections::BTreeSet;
use std::path::Path;

/// Validate coverage between the manifest, its roots, and the specification
/// tree. Diagnostics are ordered deterministically.
pub fn check(root: &Path, manifest: &Manifest, features: &[String]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let manifest_path = root.join(MANIFEST_RELATIVE_PATH);

    let matched = glob::expand(root, &manifest.roots);
    let declared: BTreeSet<&str> = manifest
        .surfaces
        .iter()
        .map(|surface| surface.path.as_str())
        .collect();

    for path in &matched {
        if !declared.contains(path.as_str()) {
            diagnostics.push(
                Diagnostic::new(
                    "UnregisteredDocumentationSurface",
                    format!(
                        "documentation root matches '{path}' but no manifest surface declares it"
                    ),
                )
                .at(root.join(path), Some(1)),
            );
        }
    }

    let mut documented: BTreeSet<String> = BTreeSet::new();
    for surface in &manifest.surfaces {
        if !root.join(&surface.path).exists() {
            diagnostics.push(
                Diagnostic::new(
                    "MissingDocumentationSurface",
                    format!("manifest surface '{}' does not exist", surface.path),
                )
                .at(manifest_path.clone(), Some(1)),
            );
        }
        documented.extend(surface.features.iter().cloned());
    }

    let known: BTreeSet<&str> = features.iter().map(String::as_str).collect();
    for feature in &documented {
        if !known.contains(feature.as_str()) {
            diagnostics.push(
                Diagnostic::new(
                    "UnknownDocumentedFeature",
                    format!("manifest documents unknown feature '{feature}'"),
                )
                .at(manifest_path.clone(), Some(1)),
            );
        }
    }
    for feature in features {
        if !documented.contains(feature) {
            diagnostics.push(
                Diagnostic::new(
                    "UndocumentedFeature",
                    format!("specification feature '{feature}' is not documented by any surface"),
                )
                .at(manifest_path.clone(), Some(1)),
            );
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempRoot;

    fn surface(path: &str, features: &[&str]) -> crate::Surface {
        crate::Surface {
            path: path.to_string(),
            features: features
                .iter()
                .map(|feature| (*feature).to_string())
                .collect(),
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-CONSISTENCY-COVERAGE-003"))]
    fn root_matches_must_be_registered() {
        let fixture = TempRoot::new("coverage-unregistered");
        fixture.write("README.md", "# Docs\n");
        let manifest = Manifest {
            roots: vec!["README.md".to_string()],
            surfaces: Vec::new(),
        };

        let diagnostics = check(fixture.path(), &manifest, &[]);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "UnregisteredDocumentationSurface"),
            "{diagnostics:?}"
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-CONSISTENCY-COVERAGE-004"))]
    fn registered_surfaces_must_exist() {
        let fixture = TempRoot::new("coverage-missing");
        let manifest = Manifest {
            roots: Vec::new(),
            surfaces: vec![surface("docs/missing.md", &[])],
        };

        let diagnostics = check(fixture.path(), &manifest, &[]);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "MissingDocumentationSurface"),
            "{diagnostics:?}"
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-CONSISTENCY-COVERAGE-005"))]
    fn documented_features_must_exist() {
        let fixture = TempRoot::new("coverage-unknown");
        fixture.write("README.md", "# Docs\n");
        let manifest = Manifest {
            roots: vec!["README.md".to_string()],
            surfaces: vec![surface("README.md", &["NOPE/NOPE"])],
        };
        let features = vec!["REAL/FEATURE".to_string()];

        let diagnostics = check(fixture.path(), &manifest, &features);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "UnknownDocumentedFeature"),
            "{diagnostics:?}"
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-CONSISTENCY-COVERAGE-006"))]
    fn every_feature_must_be_documented() {
        let fixture = TempRoot::new("coverage-undocumented");
        fixture.write("README.md", "# Docs\n");
        let manifest = Manifest {
            roots: vec!["README.md".to_string()],
            surfaces: vec![surface("README.md", &[])],
        };
        let features = vec!["REAL/FEATURE".to_string()];

        let diagnostics = check(fixture.path(), &manifest, &features);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "UndocumentedFeature"),
            "{diagnostics:?}"
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-CONSISTENCY-COVERAGE-008"))]
    fn manifest_under_bloomery_resolves_paths_against_root() {
        let fixture = TempRoot::new("coverage-root-relative");
        fixture.write("docs/a.md", "# a\n");
        fixture.write(
            ".bloomery/docs.manifest.toml",
            "roots = [\"docs/*.md\"]\n\n[[surfaces]]\npath = \"docs/a.md\"\nfeatures = [\"REAL/FEATURE\"]\n",
        );

        let manifest = crate::load_manifest(fixture.path()).expect("manifest should load");
        let diagnostics = check(fixture.path(), &manifest, &["REAL/FEATURE".to_string()]);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }
}
