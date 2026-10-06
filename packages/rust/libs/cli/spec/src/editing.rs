use crate::{RecordDetail, RecordSummary, SpecError, SpecOutcome, display_path, find_requirement};
use bloomery_model::{Area, Context, Feature, Requirement};
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table};

struct ParsedId<'a> {
    area: &'a str,
    feature: &'a str,
    group: &'a str,
}

pub(crate) fn list(
    context: &Context,
    area: Option<&str>,
    feature: Option<&str>,
    group: Option<&str>,
) -> Vec<RecordSummary> {
    let mut records = context
        .requirements()
        .filter(|(area_node, feature_node, requirement)| {
            matches(area, &area_node.id)
                && matches(feature, &feature_node.id)
                && matches(group, &requirement.group)
        })
        .map(|(area_node, feature_node, requirement)| summary(area_node, feature_node, requirement))
        .collect::<Vec<_>>();
    records.sort_by(|left, right| {
        (
            left.area.as_str(),
            left.feature.as_str(),
            left.group.as_str(),
            sequence(&left.id),
        )
            .cmp(&(
                right.area.as_str(),
                right.feature.as_str(),
                right.group.as_str(),
                sequence(&right.id),
            ))
    });
    records
}

pub(crate) fn show(context: &Context, id: &str) -> Result<RecordDetail, SpecError> {
    let (area, feature, requirement) = find_requirement(context, id)?;
    let summary = summary(area, feature, requirement);
    Ok(RecordDetail {
        area: summary.area,
        feature: summary.feature,
        group: summary.group,
        id: summary.id,
        title: summary.title,
        manual: summary.manual,
        design_ref: summary.design_ref,
        statement: requirement.entry.ears.to_statement(),
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn add(
    root: &Path,
    context: &Context,
    id: &str,
    title: &str,
    ears: &str,
    design: Option<&str>,
    manual: bool,
) -> Result<SpecOutcome, SpecError> {
    let parsed = parse_id(id)?;
    if find_requirement(context, id).is_ok() {
        return Err(SpecError::usage(format!(
            "requirement ID '{id}' already exists"
        )));
    }
    let specs_root = context
        .config
        .specs_root(root)
        .map_err(SpecError::from_diagnostic)?;
    let area_dir = specs_root.join(parsed.area);
    let feature_dir = area_dir.join(parsed.feature);
    if !area_dir.join("README.md").is_file() {
        return Err(SpecError::usage(format!(
            "area '{}' has no README at {}",
            parsed.area,
            display_path(root, &area_dir.join("README.md"))
        )));
    }
    if !feature_dir.join("README.md").is_file() {
        return Err(SpecError::usage(format!(
            "feature '{}' has no README at {}",
            parsed.feature,
            display_path(root, &feature_dir.join("README.md"))
        )));
    }
    let requirements_dir = feature_dir.join("requirements");
    if !requirements_dir.is_dir() {
        return Err(SpecError::usage(format!(
            "feature '{}' has no requirements directory",
            parsed.feature
        )));
    }
    let group_path = requirements_dir.join(format!("{}.toml", parsed.group));
    let existed = group_path.is_file();
    let mut document = if existed {
        read_document(&group_path)?
    } else {
        let mut document = DocumentMut::new();
        document["group"] = toml_edit::value(parsed.group);
        document
    };
    let record = record_table(id, title, manual, design, ears)?;
    insert_record(&mut document, record)?;
    let candidate = document.to_string();
    publish_validated(root, &group_path, &candidate, existed)?;
    Ok(SpecOutcome::Add {
        id: id.to_owned(),
        path: display_path(root, &group_path),
        created_group: !existed,
    })
}

pub(crate) fn set(
    root: &Path,
    context: &Context,
    id: &str,
    field: &str,
    value: &str,
) -> Result<SpecOutcome, SpecError> {
    let (_, _, requirement) = find_requirement(context, id)?;
    let group_path = requirement.path.clone();
    let mut document = read_document(&group_path)?;
    let table = find_table_mut(&mut document, id)?.ok_or_else(|| {
        SpecError::failure(format!("requirement '{id}' is missing from its group file"))
    })?;
    match field {
        "title" => table["title"] = toml_edit::value(value),
        "manual" => {
            let manual = value
                .parse::<bool>()
                .map_err(|_| SpecError::usage("manual must be 'true' or 'false'"))?;
            table["manual"] = toml_edit::value(manual);
        }
        "design" => {
            if value.is_empty() {
                table.remove("design");
            } else {
                table["design"] = toml_edit::value(value);
            }
        }
        "ears" => table["ears"] = parse_ears(value)?,
        other => {
            return Err(SpecError::usage(format!(
                "unknown field '{other}'; expected title, manual, design, or ears"
            )));
        }
    }
    let candidate = document.to_string();
    publish_validated(root, &group_path, &candidate, true)?;
    Ok(SpecOutcome::Set {
        id: id.to_owned(),
        field: field.to_owned(),
        changed: true,
    })
}

pub(crate) fn remove(root: &Path, context: &Context, id: &str) -> Result<SpecOutcome, SpecError> {
    let (_, feature, requirement) = find_requirement(context, id)?;
    let group_path = requirement.path.clone();
    let mut document = read_document(&group_path)?;
    let array = document
        .get_mut("requirements")
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| SpecError::failure("requirement group has no [[requirements]] array"))?;
    let index = array
        .iter()
        .position(|table| table.get("id").and_then(Item::as_str) == Some(id))
        .ok_or_else(|| {
            SpecError::failure(format!("requirement '{id}' is missing from its group file"))
        })?;
    array.remove(index);
    if array.is_empty() {
        if feature.groups.len() > 1 {
            remove_validated(root, &group_path)?;
            return Ok(SpecOutcome::Remove {
                id: id.to_owned(),
                pruned_group: true,
            });
        }
        return Err(SpecError::usage(format!(
            "cannot remove '{id}': feature '{}' must keep at least one requirement group",
            feature.id
        )));
    }
    let candidate = document.to_string();
    publish_validated(root, &group_path, &candidate, true)?;
    Ok(SpecOutcome::Remove {
        id: id.to_owned(),
        pruned_group: false,
    })
}

fn matches(filter: Option<&str>, value: &str) -> bool {
    filter.is_none_or(|filter| filter == value)
}

fn summary(area: &Area, feature: &Feature, requirement: &Requirement) -> RecordSummary {
    RecordSummary {
        area: area.id.clone(),
        feature: feature.id.clone(),
        group: requirement.group.clone(),
        id: requirement.entry.id.clone(),
        title: requirement.entry.title.clone(),
        manual: requirement.entry.manual,
        design_ref: design_display(requirement),
    }
}

fn design_display(requirement: &Requirement) -> String {
    match &requirement.design.anchor {
        Some(anchor) => format!("{}#{anchor}", requirement.design.display),
        None => requirement.design.display.clone(),
    }
}

fn sequence(id: &str) -> u32 {
    id.rsplit('-')
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

fn parse_id(id: &str) -> Result<ParsedId<'_>, SpecError> {
    let parts = id.split('-').collect::<Vec<_>>();
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
        return Err(SpecError::usage(format!(
            "requirement ID '{id}' does not match AREA-FEATURE-GROUP-SEQUENCE"
        )));
    }
    Ok(ParsedId {
        area: parts[0],
        feature: parts[1],
        group: parts[2],
    })
}

fn record_table(
    id: &str,
    title: &str,
    manual: bool,
    design: Option<&str>,
    ears: &str,
) -> Result<Table, SpecError> {
    let mut table = Table::new();
    table["id"] = toml_edit::value(id);
    table["title"] = toml_edit::value(title);
    table["manual"] = toml_edit::value(manual);
    if let Some(design) = design {
        table["design"] = toml_edit::value(design);
    }
    table["ears"] = parse_ears(ears)?;
    Ok(table)
}

fn parse_ears(ears: &str) -> Result<Item, SpecError> {
    let document: DocumentMut = format!("ears = {ears}")
        .parse()
        .map_err(|error| SpecError::usage(format!("invalid EARS table: {error}")))?;
    document
        .get("ears")
        .cloned()
        .ok_or_else(|| SpecError::usage("invalid EARS table"))
}

fn insert_record(document: &mut DocumentMut, record: Table) -> Result<(), SpecError> {
    let new_sequence = record
        .get("id")
        .and_then(Item::as_str)
        .map(sequence)
        .unwrap_or(0);
    let array = document
        .get_mut("requirements")
        .and_then(Item::as_array_of_tables_mut);
    if let Some(array) = array {
        let index = array
            .iter()
            .position(|table| {
                table
                    .get("id")
                    .and_then(Item::as_str)
                    .map(sequence)
                    .is_some_and(|existing| existing > new_sequence)
            })
            .unwrap_or(array.len());
        array.insert(index, record);
        return Ok(());
    }
    let mut array = ArrayOfTables::new();
    array.push(record);
    document["requirements"] = Item::ArrayOfTables(array);
    Ok(())
}

fn find_table_mut<'a>(
    document: &'a mut DocumentMut,
    id: &str,
) -> Result<Option<&'a mut Table>, SpecError> {
    let array = document
        .get_mut("requirements")
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| SpecError::failure("requirement group has no [[requirements]] array"))?;
    Ok(array
        .iter_mut()
        .find(|table| table.get("id").and_then(Item::as_str) == Some(id)))
}

fn read_document(path: &Path) -> Result<DocumentMut, SpecError> {
    let contents = fs::read_to_string(path).map_err(|error| {
        SpecError::failure(format!("unable to read {}: {error}", path.display()))
    })?;
    contents
        .parse()
        .map_err(|error| SpecError::failure(format!("unable to parse {}: {error}", path.display())))
}

fn publish_validated(
    root: &Path,
    target: &Path,
    candidate: &str,
    existed: bool,
) -> Result<(), SpecError> {
    let backup = sibling(target, "bloomery-backup");
    if existed {
        fs::rename(target, &backup).map_err(|error| {
            SpecError::failure(format!("unable to stage {}: {error}", target.display()))
        })?;
    }
    let temporary = sibling(target, "bloomery-tmp");
    if let Err(error) = fs::write(&temporary, candidate) {
        restore(target, &backup, existed);
        return Err(SpecError::failure(format!(
            "unable to write {}: {error}",
            temporary.display()
        )));
    }
    if let Err(error) = fs::rename(&temporary, target) {
        restore(target, &backup, existed);
        return Err(SpecError::failure(format!(
            "unable to publish {}: {error}",
            target.display()
        )));
    }
    match bloomery_workspace::load(root) {
        Ok(_) => {
            if existed {
                let _ = fs::remove_file(&backup);
            }
            Ok(())
        }
        Err(diagnostics) => {
            let _ = fs::remove_file(target);
            restore(target, &backup, existed);
            Err(SpecError::usage(format!(
                "mutation produced an invalid specification tree: {}",
                diagnostics
                    .first()
                    .map(|diagnostic| diagnostic.message.as_str())
                    .unwrap_or("unknown error")
            )))
        }
    }
}

fn remove_validated(root: &Path, target: &Path) -> Result<(), SpecError> {
    let backup = sibling(target, "bloomery-backup");
    fs::rename(target, &backup).map_err(|error| {
        SpecError::failure(format!("unable to stage {}: {error}", target.display()))
    })?;
    match bloomery_workspace::load(root) {
        Ok(_) => {
            let _ = fs::remove_file(&backup);
            Ok(())
        }
        Err(diagnostics) => {
            let _ = fs::rename(&backup, target);
            Err(SpecError::usage(format!(
                "mutation produced an invalid specification tree: {}",
                diagnostics
                    .first()
                    .map(|diagnostic| diagnostic.message.as_str())
                    .unwrap_or("unknown error")
            )))
        }
    }
}

fn restore(target: &Path, backup: &Path, existed: bool) {
    let _ = fs::remove_file(target);
    if existed {
        let _ = fs::rename(backup, target);
    }
    let _ = fs::remove_file(sibling(target, "bloomery-tmp"));
}

fn sibling(target: &Path, suffix: &str) -> PathBuf {
    let name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    target.with_file_name(format!(".{name}.{suffix}"))
}
