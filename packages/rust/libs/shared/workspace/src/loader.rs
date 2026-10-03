use crate::markdown;
use bloomery_model::{
    Area, Context, DesignReference, Diagnostic, Feature, MarkdownFrontmatter, Requirement,
    RequirementEntry, RequirementGroup, RequirementGroupFile, config,
};
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub fn load(root: &Path) -> Result<Context, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let configuration = match config::load(root) {
        Ok(configuration) => configuration,
        Err(diagnostic) => return Err(vec![diagnostic]),
    };
    let specs_root = match configuration.specs_root(root) {
        Ok(path) => path,
        Err(diagnostic) => return Err(vec![diagnostic]),
    };
    if !specs_root.is_dir() {
        return Err(vec![
            Diagnostic::new(
                "MissingSpecsDirectory",
                format!("Specs directory does not exist: {}", specs_root.display()),
            )
            .at(specs_root, Some(1)),
        ]);
    }

    let area_paths = match child_directories(&specs_root) {
        Ok(paths) => paths,
        Err(diagnostic) => return Err(vec![diagnostic]),
    };
    let mut areas = Vec::new();
    let mut all_ids = HashSet::new();
    for area_path in area_paths {
        if let Some(area) = load_area(root, &area_path, &mut all_ids, &mut diagnostics) {
            areas.push(area);
        }
    }
    areas.sort_by(|left, right| left.id.cmp(&right.id));
    if diagnostics.is_empty() {
        Ok(Context {
            root: root.to_path_buf(),
            config: configuration,
            areas,
        })
    } else {
        Err(diagnostics)
    }
}

fn load_area(
    root: &Path,
    path: &Path,
    all_ids: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Area> {
    let readme = path.join("README.md");
    let frontmatter = read_frontmatter(&readme, diagnostics)?;
    let id = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned();
    if frontmatter.id != id {
        diagnostics.push(
            Diagnostic::new(
                "DirectoryIdMismatch",
                format!(
                    "Area directory is '{id}', but frontmatter id is '{}'.",
                    frontmatter.id
                ),
            )
            .at(readme.clone(), Some(2)),
        );
    }
    let feature_paths = match child_directories(path) {
        Ok(paths) => paths,
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            return None;
        }
    };
    let mut features = Vec::new();
    for feature_path in feature_paths {
        if let Some(feature) = load_feature(root, path, &feature_path, all_ids, diagnostics) {
            features.push(feature);
        }
    }
    features.sort_by(|left, right| left.id.cmp(&right.id));
    Some(Area {
        id,
        path: path.to_path_buf(),
        frontmatter,
        features,
    })
}

fn load_feature(
    root: &Path,
    area_path: &Path,
    path: &Path,
    all_ids: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Feature> {
    let readme = path.join("README.md");
    let frontmatter = read_frontmatter(&readme, diagnostics)?;
    let id = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned();
    if frontmatter.id != id {
        diagnostics.push(
            Diagnostic::new(
                "DirectoryIdMismatch",
                format!(
                    "Feature directory is '{id}', but frontmatter id is '{}'.",
                    frontmatter.id
                ),
            )
            .at(readme.clone(), Some(2)),
        );
    }
    let design_dir = path.join("design");
    if !design_dir.is_dir() {
        diagnostics.push(
            Diagnostic::new(
                "MissingDesignDirectory",
                "Every feature must contain a design directory",
            )
            .at(path.to_path_buf(), Some(1)),
        );
    }
    let requirements_dir = path.join("requirements");
    let group_paths = match requirement_paths(&requirements_dir) {
        Ok(paths) if !paths.is_empty() => paths,
        Ok(_) => {
            diagnostics.push(
                Diagnostic::new(
                    "MissingRequirementGroup",
                    "Every feature must contain at least one .toml requirement group",
                )
                .at(requirements_dir.clone(), Some(1)),
            );
            Vec::new()
        }
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            Vec::new()
        }
    };
    let mut groups = Vec::new();
    for group_path in group_paths {
        if let Some(group) = load_group(
            root,
            area_path,
            path,
            &readme,
            &group_path,
            all_ids,
            diagnostics,
        ) {
            groups.push(group);
        }
    }
    groups.sort_by(|left, right| left.group.cmp(&right.group));
    Some(Feature {
        id,
        path: path.to_path_buf(),
        frontmatter,
        groups,
    })
}

fn load_group(
    root: &Path,
    area_path: &Path,
    feature_path: &Path,
    readme: &Path,
    path: &Path,
    all_ids: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<RequirementGroup> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) => {
            diagnostics.push(
                Diagnostic::new(
                    "ParseError",
                    format!("Unable to read requirement group: {error}"),
                )
                .at(path.to_path_buf(), Some(1)),
            );
            return None;
        }
    };
    let parsed: RequirementGroupFile = match toml::from_str(&contents) {
        Ok(parsed) => parsed,
        Err(error) => {
            diagnostics.push(
                Diagnostic::new(
                    "ParseError",
                    format!("Unable to parse requirement group: {error}"),
                )
                .at(path.to_path_buf(), Some(1)),
            );
            return None;
        }
    };
    let group = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_owned();
    if parsed.group != group {
        diagnostics.push(
            Diagnostic::new(
                "GroupNameMismatch",
                format!(
                    "TOML group '{}' does not match file stem '{group}'.",
                    parsed.group
                ),
            )
            .at(path.to_path_buf(), Some(1)),
        );
    }
    let mut sequences = BTreeSet::new();
    let mut requirements = Vec::new();
    for entry in parsed.requirements {
        let line = markdown::line_containing(&contents, &format!("\"{}\"", entry.id));
        if let Some(design) = validate_requirement(
            root,
            area_path,
            feature_path,
            readme,
            &group,
            &entry,
            path,
            line,
            &mut sequences,
            all_ids,
            diagnostics,
        ) {
            requirements.push(Requirement {
                entry,
                group: group.clone(),
                path: path.to_path_buf(),
                line,
                design,
            });
        }
    }
    Some(RequirementGroup {
        group,
        path: path.to_path_buf(),
        requirements,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_requirement(
    root: &Path,
    area_path: &Path,
    feature_path: &Path,
    readme: &Path,
    group: &str,
    entry: &RequirementEntry,
    group_path: &Path,
    line: usize,
    sequences: &mut BTreeSet<String>,
    all_ids: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<DesignReference> {
    let area = area_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let feature = feature_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let parts: Vec<&str> = entry.id.split('-').collect();
    let valid = parts.len() == 4
        && parts[..3].iter().all(|part| {
            !part.is_empty()
                && part.chars().all(|character| {
                    character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
                })
        })
        && (parts[3].len() == 3 || parts[3].len() == 4)
        && parts[3].chars().all(|character| character.is_ascii_digit());
    if !valid {
        diagnostics.push(
            Diagnostic::new(
                "InvalidSpecId",
                "Requirement ID does not match the required four-segment grammar",
            )
            .at(group_path.to_path_buf(), Some(line)),
        );
    } else {
        let expected = format!("{area}-{feature}-{group}-{}", parts[3]);
        if entry.id != expected {
            diagnostics.push(
                Diagnostic::new(
                    "SpecIdPathMismatch",
                    format!("Expected requirement ID prefix '{area}-{feature}-{group}-'"),
                )
                .at(group_path.to_path_buf(), Some(line)),
            );
        }
        if !sequences.insert(parts[3].to_owned()) {
            diagnostics.push(
                Diagnostic::new(
                    "DuplicateSequence",
                    "Requirement sequence is duplicated within its group",
                )
                .at(group_path.to_path_buf(), Some(line)),
            );
        }
        if !all_ids.insert(entry.id.clone()) {
            diagnostics.push(
                Diagnostic::new(
                    "DuplicateSpecId",
                    "Requirement ID is declared more than once",
                )
                .at(group_path.to_path_buf(), Some(line)),
            );
        }
    }

    let (relative_path, anchor) = entry
        .design
        .as_deref()
        .map(split_design_reference)
        .unwrap_or_else(|| ("README.md".to_owned(), None));
    let relative = Path::new(&relative_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| component == Component::ParentDir)
    {
        diagnostics.push(
            Diagnostic::new(
                "InvalidDesignPath",
                "Design references must stay inside the feature directory",
            )
            .at(group_path.to_path_buf(), Some(line)),
        );
        return None;
    }
    let design_path = feature_path.join(relative);
    if !design_path.is_file() {
        diagnostics.push(
            Diagnostic::new(
                "MissingDesignFile",
                "Requirement references a design document that does not exist",
            )
            .at(group_path.to_path_buf(), Some(line))
            .note(format!("File not found: {}", design_path.display())),
        );
        return None;
    }
    if entry.design.is_none() && design_path != readme {
        diagnostics.push(
            Diagnostic::new(
                "DesignResolutionError",
                "Default design reference did not resolve to the feature README",
            )
            .at(group_path.to_path_buf(), Some(line)),
        );
    }
    if let Some(anchor) = &anchor {
        match markdown::has_anchor(&design_path, anchor) {
            Ok(true) => {}
            Ok(false) => diagnostics.push(
                Diagnostic::new(
                    "DanglingDesignAnchor",
                    "Heading anchor was not found in the target design document",
                )
                .at(group_path.to_path_buf(), Some(line))
                .note(format!("Anchor not found: #{anchor}")),
            ),
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }
    Some(DesignReference {
        path: design_path.clone(),
        display: display_path(root, &design_path),
        anchor,
    })
}

fn split_design_reference(reference: &str) -> (String, Option<String>) {
    match reference.split_once('#') {
        Some((path, anchor)) => (path.to_owned(), Some(anchor.to_owned())),
        None => (reference.to_owned(), None),
    }
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|path| {
            path.to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/")
        })
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

fn read_frontmatter(path: &Path, diagnostics: &mut Vec<Diagnostic>) -> Option<MarkdownFrontmatter> {
    match markdown::read_frontmatter(path) {
        Ok(frontmatter) => Some(frontmatter),
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            None
        }
    }
}

fn child_directories(path: &Path) -> Result<Vec<PathBuf>, Diagnostic> {
    let entries = fs::read_dir(path).map_err(|error| {
        Diagnostic::new(
            "WorkspaceReadError",
            format!("Unable to read workspace directory: {error}"),
        )
        .at(path.to_path_buf(), Some(1))
    })?;
    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn requirement_paths(path: &Path) -> Result<Vec<PathBuf>, Diagnostic> {
    if !path.is_dir() {
        return Err(Diagnostic::new(
            "MissingRequirementsDirectory",
            "Every feature must contain a requirements directory",
        )
        .at(path.to_path_buf(), Some(1)));
    }
    let entries = fs::read_dir(path).map_err(|error| {
        Diagnostic::new(
            "WorkspaceReadError",
            format!("Unable to read requirements directory: {error}"),
        )
        .at(path.to_path_buf(), Some(1))
    })?;
    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension == "toml")
        })
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}
