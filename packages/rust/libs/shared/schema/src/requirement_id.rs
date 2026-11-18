//! Requirement identifier grammar: `AREA-FEATURE-GROUP-SEQUENCE`.

/// Parsed segments of a requirement identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequirementId<'a> {
    pub area: &'a str,
    pub feature: &'a str,
    pub group: &'a str,
    pub sequence: &'a str,
}

/// Parse `id` when it has three non-empty segments of uppercase ASCII letters,
/// digits, or `_`, followed by a three- or four-digit sequence.
pub fn parse_requirement_id(id: &str) -> Option<RequirementId<'_>> {
    let mut parts = id.split('-');
    let area = parts.next()?;
    let feature = parts.next()?;
    let group = parts.next()?;
    let sequence = parts.next()?;
    let valid = parts.next().is_none()
        && [area, feature, group].into_iter().all(valid_segment)
        && matches!(sequence.len(), 3 | 4)
        && sequence.bytes().all(|byte| byte.is_ascii_digit());
    valid.then_some(RequirementId {
        area,
        feature,
        group,
        sequence,
    })
}

/// Whether `id` matches the requirement identifier grammar.
pub fn is_requirement_id(id: &str) -> bool {
    parse_requirement_id(id).is_some()
}

/// Whether `value` is a non-empty identifier segment of uppercase ASCII
/// letters, digits, or `_`.
pub fn valid_segment(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::{is_requirement_id, parse_requirement_id};

    #[test]
    fn requirement_ids_follow_the_four_segment_grammar() {
        let parsed = parse_requirement_id("CLI-CHECK_X-G2-0012").expect("valid");
        assert_eq!(
            (parsed.area, parsed.feature, parsed.group, parsed.sequence),
            ("CLI", "CHECK_X", "G2", "0012")
        );
        assert!(is_requirement_id("A-B-C-001"));
        for invalid in [
            "A-B-C-01",
            "A-B-C-00001",
            "A-B-C",
            "A-B-C-001-002",
            "a-B-C-001",
            "A--C-001",
            "A-B-C-00a",
        ] {
            assert!(!is_requirement_id(invalid), "{invalid}");
        }
    }
}
