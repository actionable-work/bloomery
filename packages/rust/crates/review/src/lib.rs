use bloomery_model::{Context, Requirement};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ReviewItem {
    pub service: String,
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
        .map(|(service, feature, requirement)| ReviewItem {
            service: service.id.clone(),
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
            left.service.as_str(),
            left.feature.as_str(),
            left.group.as_str(),
            sequence(&left.id),
        )
            .cmp(&(
                right.service.as_str(),
                right.feature.as_str(),
                right.group.as_str(),
                sequence(&right.id),
            ))
    });
    items
}

pub fn render_text(items: &[ReviewItem]) -> String {
    let mut output = String::new();
    let mut previous_service = None;
    let mut previous_feature = None;
    let mut previous_group = None;
    for item in items {
        if previous_service != Some(item.service.as_str()) {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&item.service);
            output.push('\n');
            previous_service = Some(item.service.as_str());
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
