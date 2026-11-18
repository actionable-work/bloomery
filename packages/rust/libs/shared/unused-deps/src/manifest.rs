//! Locate dependency declarations in a Cargo manifest.

use bloomery_cargo_metadata::{Dependency, DependencyKind};
use toml_edit::{Document, Item, TableLike};

/// One-based line of the manifest key that declares `dependency`, searching
/// the top-level section for its kind and then each `[target.*]` section.
pub(crate) fn declaration_line(manifest: &str, dependency: &Dependency) -> Option<usize> {
    let document = Document::parse(manifest).ok()?;
    let sections: &[&str] = match dependency.kind() {
        DependencyKind::Normal => &["dependencies"],
        DependencyKind::Development => &["dev-dependencies", "dev_dependencies"],
        DependencyKind::Build => &["build-dependencies", "build_dependencies"],
    };
    let root = document.as_table() as &dyn TableLike;
    let platforms = root
        .get("target")
        .and_then(Item::as_table_like)
        .into_iter()
        .flat_map(|targets| targets.iter())
        .filter_map(|(_, platform)| platform.as_table_like());
    let start = std::iter::once(root)
        .chain(platforms)
        .flat_map(|table| sections.iter().filter_map(|name| table.get(name)))
        .filter_map(Item::as_table_like)
        .find_map(|section| section.get_key_value(dependency.manifest_key()))
        .and_then(|(key, _)| key.span())?
        .start;
    Some(manifest[..start].matches('\n').count() + 1)
}

#[cfg(test)]
mod tests {
    use super::declaration_line;
    use bloomery_cargo_metadata::Dependency;

    fn dependency(name: &str, rename: Option<&str>, kind: Option<&str>) -> Dependency {
        Dependency {
            name: name.to_owned(),
            rename: rename.map(str::to_owned),
            kind: kind.map(str::to_owned),
            target: None,
        }
    }

    const MANIFEST: &str = "[package]\nname = \"member\"\n\n[dependencies]\nserde = \"1\"\njson = { package = \"serde_json\", version = \"1\" }\n\n[dev-dependencies.tempfile]\nversion = \"3\"\n\n[target.'cfg(unix)'.build-dependencies]\ncc = \"1\"\n";

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-009"))]
    fn finds_inline_dotted_renamed_and_platform_declarations() {
        assert_eq!(
            declaration_line(MANIFEST, &dependency("serde", None, None)),
            Some(5)
        );
        assert_eq!(
            declaration_line(MANIFEST, &dependency("serde_json", Some("json"), None)),
            Some(6)
        );
        assert_eq!(
            declaration_line(MANIFEST, &dependency("tempfile", None, Some("dev"))),
            Some(8)
        );
        assert_eq!(
            declaration_line(MANIFEST, &dependency("cc", None, Some("build"))),
            Some(12)
        );
        assert_eq!(
            declaration_line(MANIFEST, &dependency("serde", None, Some("dev"))),
            None
        );
    }
}
