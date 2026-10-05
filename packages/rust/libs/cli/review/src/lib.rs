use bloomery_model::{Context, Requirement};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ReviewItem {
    pub area: String,
    pub feature: String,
    pub group: String,
    pub id: String,
    pub title: String,
    pub statement: String,
    pub design_ref: String,
}

pub fn items(context: &Context) -> Vec<ReviewItem> {
    let mut items = context
        .requirements()
        .filter(|(_, _, requirement)| requirement.entry.manual)
        .map(|(area, feature, requirement)| ReviewItem {
            area: area.id.clone(),
            feature: feature.id.clone(),
            group: requirement.group.clone(),
            id: requirement.entry.id.clone(),
            title: requirement.entry.title.clone(),
            statement: requirement.entry.ears.to_statement(),
            design_ref: design_display(requirement),
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
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
    items
}

pub fn render_text(items: &[ReviewItem]) -> String {
    let mut output = String::new();
    let mut previous_area = None;
    let mut previous_feature = None;
    let mut previous_group = None;
    for item in items {
        if previous_area != Some(item.area.as_str()) {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&item.area);
            output.push('\n');
            previous_area = Some(item.area.as_str());
            previous_feature = None;
            previous_group = None;
        }
        if previous_feature != Some(item.feature.as_str()) {
            output.push_str(&format!("└── {}\n", item.feature));
            previous_feature = Some(item.feature.as_str());
            previous_group = None;
        }
        if previous_group != Some(item.group.as_str()) {
            output.push_str(&format!("    └── {}\n", item.group));
            previous_group = Some(item.group.as_str());
        }
        output.push_str(&format!("        └── [{}] {}\n", item.id, item.title));
        output.push_str(&format!("            {}\n", item.statement));
        output.push_str(&format!("            Design: {}\n", item.design_ref));
    }
    output.push_str(&format!(
        "\nTotal manual requirements requiring review: {}\n",
        items.len()
    ));
    output
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

#[cfg(test)]
mod tests {
    use super::items;
    use bloomery_model::{
        Area, Config, Context, DesignReference, EarsStatement, Feature, MarkdownFrontmatter,
        Requirement, RequirementEntry, RequirementGroup,
    };
    use std::path::PathBuf;

    fn context(requirements: &[(&str, bool)]) -> Context {
        let mut areas: Vec<Area> = Vec::new();
        for (id, manual) in requirements {
            let mut parts = id.splitn(4, '-');
            let area_id = parts.next().expect("area ID");
            let feature_id = parts.next().expect("feature ID");
            let group_id = parts.next().expect("group ID");
            let _sequence = parts.next().expect("sequence");
            let area_index = areas
                .iter()
                .position(|area| area.id == area_id)
                .unwrap_or_else(|| {
                    areas.push(Area {
                        id: area_id.to_owned(),
                        path: PathBuf::from(area_id),
                        frontmatter: frontmatter(area_id),
                        features: Vec::new(),
                    });
                    areas.len() - 1
                });
            let feature_index = areas[area_index]
                .features
                .iter()
                .position(|feature| feature.id == feature_id)
                .unwrap_or_else(|| {
                    areas[area_index].features.push(Feature {
                        id: feature_id.to_owned(),
                        path: PathBuf::from(area_id).join(feature_id),
                        frontmatter: frontmatter(feature_id),
                        groups: Vec::new(),
                    });
                    areas[area_index].features.len() - 1
                });
            let groups = &mut areas[area_index].features[feature_index].groups;
            let group_index = groups
                .iter()
                .position(|group| group.group == group_id)
                .unwrap_or_else(|| {
                    groups.push(RequirementGroup {
                        group: group_id.to_owned(),
                        path: PathBuf::from(format!("{group_id}.toml")),
                        requirements: Vec::new(),
                    });
                    groups.len() - 1
                });
            groups[group_index].requirements.push(Requirement {
                entry: RequirementEntry {
                    id: id.to_string(),
                    title: format!("Requirement {id}"),
                    manual: *manual,
                    design: Some("design/test.md".to_owned()),
                    ears: EarsStatement::Ubiquitous {
                        system: "the review catalogue".to_owned(),
                        action: "list manual requirements".to_owned(),
                    },
                },
                group: group_id.to_owned(),
                path: PathBuf::from(format!("{group_id}.toml")),
                line: 1,
                design: DesignReference {
                    path: PathBuf::from("design/test.md"),
                    display: "design/test.md".to_owned(),
                    anchor: None,
                },
            });
        }
        Context {
            root: PathBuf::new(),
            config: Config::default(),
            areas,
        }
    }

    fn frontmatter(id: &str) -> MarkdownFrontmatter {
        MarkdownFrontmatter {
            id: id.to_owned(),
            name: id.to_owned(),
            tagline: id.to_owned(),
            description: id.to_owned(),
        }
    }

    fn ids(context: &Context) -> Vec<String> {
        items(context).into_iter().map(|item| item.id).collect()
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-REVIEW-CATALOG-001"))]
    fn catalogue_contains_all_and_only_manual_requirements() {
        let context = context(&[
            ("ALPHA-ONE-GRP-001", true),
            ("ALPHA-ONE-GRP-002", false),
            ("ALPHA-ONE-OTHER-001", true),
            ("BETA-TWO-GRP-003", false),
        ]);

        assert_eq!(
            ids(&context),
            vec![
                "ALPHA-ONE-GRP-001".to_owned(),
                "ALPHA-ONE-OTHER-001".to_owned(),
            ]
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-REVIEW-CATALOG-002"))]
    fn catalogue_order_is_hierarchical_and_uses_numeric_sequences() {
        let context = context(&[
            ("ZETA-ALPHA-BETA-001", true),
            ("ALPHA-ZETA-BETA-002", true),
            ("ALPHA-ALPHA-ZETA-001", true),
            ("ALPHA-ALPHA-ALPHA-010", true),
            ("ALPHA-ALPHA-ALPHA-002", true),
            ("ALPHA-ALPHA-ALPHA-001", true),
        ]);
        let expected = vec![
            "ALPHA-ALPHA-ALPHA-001",
            "ALPHA-ALPHA-ALPHA-002",
            "ALPHA-ALPHA-ALPHA-010",
            "ALPHA-ALPHA-ZETA-001",
            "ALPHA-ZETA-BETA-002",
            "ZETA-ALPHA-BETA-001",
        ];

        assert_eq!(ids(&context), expected);
        assert_eq!(ids(&context), expected);
    }
}
