use crate::markdown;
use crate::source::{FilesystemTree, SourceTree, TreeIndex, path_key};
use bloomery_model::{
    Area, Context, DesignReference, Diagnostic, Feature, MarkdownFrontmatter, Requirement,
    RequirementEntry, RequirementGroup, RequirementGroupFile, config,
};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::{BTreeSet, HashSet};
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

/// Load repository specifications from the local filesystem.
pub fn load(root: &Path) -> Result<Context, Vec<Diagnostic>> {
    let configuration = config::load(root).map_err(|error| vec![error])?;
    let mut context =
        load_tree(&FilesystemTree::new(root), configuration).map_err(|mut errors| {
            for error in &mut errors {
                if let Some(location) = &mut error.location {
                    location.path = root.join(&location.path);
                }
            }
            errors
        })?;
    context.root = root.to_path_buf();
    for area in &mut context.areas {
        area.path = root.join(&area.path);
        for feature in &mut area.features {
            feature.path = root.join(&feature.path);
            for group in &mut feature.groups {
                group.path = root.join(&group.path);
                for requirement in &mut group.requirements {
                    requirement.path = root.join(&requirement.path);
                    requirement.design.path = root.join(&requirement.design.path);
                }
            }
        }
    }
    Ok(context)
}

/// Discover specifications solely from caller-authorized repository content.
/// All returned paths are repository-relative; no configuration file is read.
pub fn load_tree(
    tree: &dyn SourceTree,
    configuration: bloomery_model::Config,
) -> Result<Context, Vec<Diagnostic>> {
    configuration.validate().map_err(|error| vec![error])?;
    let mut context = Context {
        root: PathBuf::new(),
        config: configuration,
        specs_enabled: false,
        areas: Vec::new(),
    };
    if context.config.specs.enable == Some(false) {
        return Ok(context);
    }
    let loader = Loader {
        tree,
        index: TreeIndex::new(tree)
            .map_err(|error| vec![Diagnostic::new("SourceTreeError", error)])?,
        documents: RefCell::new(BTreeMap::new()),
    };
    let specs_root = PathBuf::from(path_key(
        &context
            .config
            .specs_root(Path::new(""))
            .map_err(|error| vec![error])?,
    ));
    if !loader.index.is_dir(&specs_root) {
        if context.config.specs.enable != Some(true) {
            return Ok(context);
        }
        return Err(vec![
            Diagnostic::new(
                "MissingSpecsDirectory",
                format!("Specs directory does not exist: {}", specs_root.display()),
            )
            .at(specs_root, Some(1)),
        ]);
    }
    context.specs_enabled = true;
    let mut diagnostics = Vec::new();
    let mut all_ids = HashSet::new();
    for area_path in loader.index.child_directories(&specs_root) {
        if let Some(area) = load_area(&loader, &area_path, &mut all_ids, &mut diagnostics) {
            context.areas.push(area);
        }
    }
    context.areas.sort_by(|a, b| a.id.cmp(&b.id));
    bloomery_model::sort_diagnostics(&mut diagnostics);
    if diagnostics.is_empty() {
        Ok(context)
    } else {
        Err(diagnostics)
    }
}

struct Loader<'a> {
    tree: &'a dyn SourceTree,
    index: TreeIndex,
    documents: RefCell<BTreeMap<PathBuf, Result<Rc<str>, String>>>,
}

impl Loader<'_> {
    fn read(&self, path: &Path, code: &str, description: &str) -> Result<Rc<str>, Diagnostic> {
        let path = PathBuf::from(path_key(path));
        let result = self
            .documents
            .borrow_mut()
            .entry(path.clone())
            .or_insert_with(|| {
                if !self.index.is_file(&path) {
                    return Err("file is not present in the repository tree".to_owned());
                }
                self.tree
                    .read(path.to_str().expect("canonical path"))
                    .map(Rc::from)
            })
            .clone();
        result.map_err(|error| {
            Diagnostic::new(code, format!("{description}: {error}")).at(path, Some(1))
        })
    }
}

fn load_area(
    loader: &Loader<'_>,
    path: &Path,
    all_ids: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Area> {
    let readme = path.join("README.md");
    let frontmatter = read_frontmatter(loader, &readme, diagnostics)?;
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
    let feature_paths = loader.index.child_directories(path);
    let mut features = Vec::new();
    for feature_path in feature_paths {
        if let Some(feature) = load_feature(loader, path, &feature_path, all_ids, diagnostics) {
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
    loader: &Loader<'_>,
    area_path: &Path,
    path: &Path,
    all_ids: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Feature> {
    let readme = path.join("README.md");
    let frontmatter = read_frontmatter(loader, &readme, diagnostics)?;
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
    if !loader.index.is_dir(&design_dir) {
        diagnostics.push(
            Diagnostic::new(
                "MissingDesignDirectory",
                "Every feature must contain a design directory",
            )
            .at(path.to_path_buf(), Some(1)),
        );
    }
    let requirements_dir = path.join("requirements");
    let group_paths = match requirement_paths(loader, &requirements_dir) {
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
            loader,
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
    loader: &Loader<'_>,
    area_path: &Path,
    feature_path: &Path,
    readme: &Path,
    path: &Path,
    all_ids: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<RequirementGroup> {
    let contents = match loader.read(path, "ParseError", "Unable to read requirement group") {
        Ok(contents) => contents,
        Err(error) => {
            diagnostics.push(error);
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
            &entry,
            RequirementValidation {
                loader,
                area_path,
                feature_path,
                readme,
                group: &group,
                group_path: path,
                line,
                sequences: &mut sequences,
                all_ids,
                diagnostics,
            },
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

struct RequirementValidation<'a> {
    loader: &'a Loader<'a>,
    area_path: &'a Path,
    feature_path: &'a Path,
    readme: &'a Path,
    group: &'a str,
    group_path: &'a Path,
    line: usize,
    sequences: &'a mut BTreeSet<String>,
    all_ids: &'a mut HashSet<String>,
    diagnostics: &'a mut Vec<Diagnostic>,
}

fn validate_requirement(
    entry: &RequirementEntry,
    validation: RequirementValidation<'_>,
) -> Option<DesignReference> {
    let RequirementValidation {
        loader,
        area_path,
        feature_path,
        readme,
        group,
        group_path,
        line,
        sequences,
        all_ids,
        diagnostics,
    } = validation;
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
    let design_path = PathBuf::from(path_key(&feature_path.join(relative)));
    if !loader.index.is_file(&design_path) {
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
    let design_contents = match loader.read(
        &design_path,
        "MissingDesignFile",
        "Unable to read design document",
    ) {
        Ok(contents) => contents,
        Err(error) => {
            diagnostics.push(error);
            return None;
        }
    };
    if let Some(anchor) = &anchor
        && !markdown::has_anchor(&design_contents, anchor)
    {
        diagnostics.push(
            Diagnostic::new(
                "DanglingDesignAnchor",
                "Heading anchor was not found in the target design document",
            )
            .at(group_path.to_path_buf(), Some(line))
            .note(format!("Anchor not found: #{anchor}")),
        );
    }
    Some(DesignReference {
        path: design_path.clone(),
        display: path_key(&design_path),
        anchor,
    })
}

fn split_design_reference(reference: &str) -> (String, Option<String>) {
    match reference.split_once('#') {
        Some((path, anchor)) => (path.to_owned(), Some(anchor.to_owned())),
        None => (reference.to_owned(), None),
    }
}

fn read_frontmatter(
    loader: &Loader<'_>,
    path: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<MarkdownFrontmatter> {
    let result = loader
        .read(path, "MissingDocument", "Unable to read Markdown document")
        .and_then(|contents| markdown::parse_frontmatter(path, &contents));
    match result {
        Ok(frontmatter) => Some(frontmatter),
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            None
        }
    }
}

fn requirement_paths(loader: &Loader<'_>, path: &Path) -> Result<Vec<PathBuf>, Diagnostic> {
    if !loader.index.is_dir(path) {
        return Err(Diagnostic::new(
            "MissingRequirementsDirectory",
            "Every feature must contain a requirements directory",
        )
        .at(path, Some(1)));
    }
    Ok(loader
        .index
        .child_files(path)
        .into_iter()
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .collect())
}
