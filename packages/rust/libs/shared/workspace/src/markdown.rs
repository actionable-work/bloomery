use bloomery_model::{Diagnostic, MarkdownFrontmatter};
use std::fs;
use std::path::Path;

pub fn read_frontmatter(path: &Path) -> Result<MarkdownFrontmatter, Diagnostic> {
    let contents = fs::read_to_string(path).map_err(|error| {
        Diagnostic::new(
            "MissingDocument",
            format!("Unable to read Markdown document: {error}"),
        )
        .at(path, Some(1))
    })?;
    let mut lines = contents.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Err(Diagnostic::new(
            "MissingFrontmatter",
            "Markdown document must begin with YAML frontmatter",
        )
        .at(path, Some(1)));
    }
    let mut yaml = String::new();
    for (index, line) in lines.enumerate() {
        if line.trim() == "---" {
            return serde_yaml::from_str(&yaml).map_err(|error| {
                Diagnostic::new(
                    "InvalidFrontmatter",
                    format!("Unable to parse YAML frontmatter: {error}"),
                )
                .at(path, Some(index + 2))
            });
        }
        yaml.push_str(line);
        yaml.push('\n');
    }
    Err(Diagnostic::new(
        "InvalidFrontmatter",
        "YAML frontmatter has no closing delimiter",
    )
    .at(path, Some(1)))
}

pub fn has_anchor(path: &Path, anchor: &str) -> Result<bool, Diagnostic> {
    let contents = fs::read_to_string(path).map_err(|error| {
        Diagnostic::new(
            "MissingDesignFile",
            format!("Unable to read design document: {error}"),
        )
        .at(path, Some(1))
    })?;
    let mut occurrences = std::collections::BTreeMap::<String, usize>::new();
    for line in contents.lines() {
        let trimmed = line.trim_start();
        let hash_count = trimmed
            .chars()
            .take_while(|character| *character == '#')
            .count();
        if (1..=6).contains(&hash_count)
            && trimmed
                .chars()
                .nth(hash_count)
                .is_some_and(char::is_whitespace)
        {
            let heading = trimmed[hash_count..].trim().trim_end_matches('#').trim();
            let base = slugify(heading);
            let count = occurrences.entry(base.clone()).or_insert(0);
            let candidate = if *count == 0 {
                base
            } else {
                format!("{base}-{}", *count)
            };
            *count += 1;
            if candidate == anchor {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut pending_dash = false;
    for character in value.chars().flat_map(|character| character.to_lowercase()) {
        if character.is_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            pending_dash = false;
            slug.push(character);
        } else if !slug.is_empty() {
            pending_dash = true;
        }
    }
    slug
}

pub fn line_containing(contents: &str, needle: &str) -> usize {
    contents
        .lines()
        .position(|line| line.contains(needle))
        .map(|line| line + 1)
        .unwrap_or(1)
}
