#[cfg(test)]
use std::io::{self, Write};

/// Increment this when the built-in recommendation catalog changes.
pub const RECOMMENDATION_CATALOG_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Recommendation {
    pub path: &'static [&'static str],
    pub benefit: &'static str,
    pub guidance: &'static str,
}

const CATALOG: &[Recommendation] = &[
    Recommendation {
        path: &["scanners", "rust", "enabled"],
        benefit: "Rust evidence scanning links annotated Rust tests to requirements.",
        guidance: "Configure repository-relative `paths` when enabling this scanner.",
    },
    Recommendation {
        path: &["scanners", "playwright", "enabled"],
        benefit: "Playwright evidence scanning links tagged browser tests to requirements.",
        guidance: "Configure repository-relative `paths` when enabling this scanner.",
    },
    Recommendation {
        path: &["scanners", "nix", "enabled"],
        benefit: "Nix evidence scanning links check metadata to requirements.",
        guidance: "Review `checks_attr` and optional `systems` when enabling this scanner.",
    },
];

pub(crate) fn missing_recommendations(raw: &toml::Value) -> Vec<&'static Recommendation> {
    let mut missing = CATALOG
        .iter()
        .filter(|recommendation| !contains_path(raw, recommendation.path))
        .collect::<Vec<_>>();
    missing.sort_by_key(|recommendation| path_string(recommendation));
    missing
}

#[cfg(test)]
pub(crate) fn write_recommendations(raw: &toml::Value, stderr: &mut impl Write) -> io::Result<()> {
    for recommendation in missing_recommendations(raw) {
        writeln!(
            stderr,
            "recommendation: {}",
            recommendation_message(recommendation)
        )?;
    }
    Ok(())
}

pub(crate) fn recommendation_key(recommendation: &Recommendation) -> String {
    path_string(recommendation)
}

#[cfg(test)]
pub(crate) fn recommendation_message(recommendation: &Recommendation) -> String {
    format!(
        "recommended feature not yet configured: `{}` — {} {}",
        path_string(recommendation),
        recommendation.benefit,
        recommendation.guidance
    )
}

fn contains_path(raw: &toml::Value, path: &[&str]) -> bool {
    let mut value = raw;
    for component in path {
        let Some(next) = value.as_table().and_then(|table| table.get(*component)) else {
            return false;
        };
        value = next;
    }
    true
}

fn path_string(recommendation: &Recommendation) -> String {
    recommendation.path.join(".")
}

#[cfg(test)]
mod tests {
    use super::{
        CATALOG, RECOMMENDATION_CATALOG_VERSION, missing_recommendations, write_recommendations,
    };
    use bloomery_test_macros::bloomery;

    fn raw(contents: &str) -> toml::Value {
        toml::from_str(contents).expect("valid TOML")
    }

    fn missing_paths(value: &toml::Value) -> Vec<String> {
        missing_recommendations(value)
            .into_iter()
            .map(|recommendation| recommendation.path.join("."))
            .collect()
    }

    #[test]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-001")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-006")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-007")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-008")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-009")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-010")]
    fn built_in_catalog_covers_each_recommended_scanner_with_guidance() {
        assert_eq!(RECOMMENDATION_CATALOG_VERSION, 1);
        assert_eq!(CATALOG.len(), 3);
        let empty = raw("");
        let notices = missing_recommendations(&empty);
        assert_eq!(
            notices
                .iter()
                .map(|recommendation| recommendation.path.join("."))
                .collect::<Vec<_>>(),
            [
                "scanners.nix.enabled",
                "scanners.playwright.enabled",
                "scanners.rust.enabled"
            ]
        );
        for notice in notices {
            assert!(!notice.benefit.is_empty());
            assert!(!notice.guidance.is_empty());
        }
    }

    #[test]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-002")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-011")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-012")]
    fn only_absent_raw_paths_are_recommended() {
        let absent_parent = raw("[scanners.rust]\n");
        assert_eq!(
            missing_paths(&absent_parent),
            [
                "scanners.nix.enabled",
                "scanners.playwright.enabled",
                "scanners.rust.enabled"
            ]
        );

        let explicit_leaf = raw("[scanners.rust]\nenabled = false\n");
        assert_eq!(
            missing_paths(&explicit_leaf),
            ["scanners.nix.enabled", "scanners.playwright.enabled"]
        );
    }

    #[test]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-013")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-014")]
    fn explicit_false_and_true_values_suppress_recommendations() {
        let explicit_values = raw("[scanners.rust]\nenabled = false\n\
             [scanners.playwright]\nenabled = false\n\
             [scanners.nix]\nenabled = true\n");
        assert!(missing_recommendations(&explicit_values).is_empty());
    }

    #[test]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-015")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-016")]
    fn dotted_keys_and_inline_tables_have_normal_toml_presence_semantics() {
        let dotted = raw("scanners.rust.enabled = false\n\
             scanners.playwright.enabled = false\n\
             scanners.nix.enabled = false\n");
        assert!(missing_recommendations(&dotted).is_empty());

        let inline = raw(
            "scanners = { rust = { enabled = false }, playwright = { enabled = false }, nix = { enabled = false } }\n",
        );
        assert!(missing_recommendations(&inline).is_empty());
    }

    #[test]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-003")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-017")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-018")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-019")]
    #[bloomery("CLI-SYNC-RECOMMENDATIONS-020")]
    fn notices_are_sorted_unique_and_repeat_without_persisted_history() {
        let empty = raw("");
        let first = missing_paths(&empty);
        let second = missing_paths(&empty);
        assert_eq!(first, second);
        assert_eq!(first.len(), 3);
        assert!(first.windows(2).all(|pair| pair[0] < pair[1]));

        let mut output = Vec::new();
        write_recommendations(&empty, &mut output).expect("render notices");
        let output = String::from_utf8(output).expect("UTF-8 notices");
        assert_eq!(
            output
                .matches("recommended feature not yet configured")
                .count(),
            3
        );
        assert!(
            output.find("scanners.nix.enabled").unwrap()
                < output.find("scanners.playwright.enabled").unwrap()
        );
        assert!(
            output.find("scanners.playwright.enabled").unwrap()
                < output.find("scanners.rust.enabled").unwrap()
        );
        assert!(!output.contains("introduced since"));
    }

    #[test]
    fn a_catalog_can_grow_without_notification_state_or_language_filtering() {
        let future_entry = super::Recommendation {
            path: &["scanners", "future", "enabled"],
            benefit: "future benefit",
            guidance: "future guidance",
        };
        let value = raw("");
        assert!(!super::contains_path(&value, future_entry.path));
        assert!(CATALOG.iter().all(|entry| {
            entry.benefit.contains("scanning") || entry.benefit.contains("evidence")
        }));
    }
}
