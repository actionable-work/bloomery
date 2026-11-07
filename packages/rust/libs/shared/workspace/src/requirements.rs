use bloomery_model::{CURRENT_REQUIREMENT_SCHEMA_VERSION, Diagnostic, RequirementGroupFile};
use std::path::Path;

/// Parse and validate one grouped source document using its original bytes.
pub(crate) fn parse(source: &str, path: &Path) -> Result<RequirementGroupFile, Diagnostic> {
    let parsed: RequirementGroupFile =
        toml::from_str(source).map_err(|error: toml::de::Error| {
            let line = error
                .span()
                .map(|span| line_at(source, span.start))
                .unwrap_or(1);
            Diagnostic::new(
                "ParseError",
                format!("Unable to parse requirement group: {error}"),
            )
            .at(path, Some(line))
        })?;
    if let Some(version) = &parsed.schema_version
        && *version.get_ref() != CURRENT_REQUIREMENT_SCHEMA_VERSION
    {
        return Err(Diagnostic::new(
            "UnsupportedSpecificationSchema",
            format!(
                "Unsupported requirement schema version {}; supported version is {CURRENT_REQUIREMENT_SCHEMA_VERSION}",
                version.get_ref()
            ),
        )
        .at(path, Some(line_at(source, version.span().start))));
    }
    Ok(parsed)
}

fn line_at(source: &str, offset: usize) -> usize {
    source
        .as_bytes()
        .iter()
        .take(offset)
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}
