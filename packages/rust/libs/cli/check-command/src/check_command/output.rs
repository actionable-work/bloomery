use super::details::normalize_display_text;
use super::model::{
    CheckRecord, FailureRecord, OUTPUT_BYTE_LIMIT, RunRecord, RunStatus, page_window, truncate_utf8,
};
use serde_json::{Map, Value, json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedPage {
    pub value: Value,
    pub bytes: Vec<u8>,
    pub offset: usize,
    pub next: Option<usize>,
}

pub fn error_document(code: &str, message: &str) -> Value {
    let (message, _) = truncate_utf8(message, 2048);
    json!({
        "command": "check",
        "status": "error",
        "error": {
            "code": code,
            "message": message,
        },
    })
}

pub fn compact_json(value: &Value) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec(value)
        .map_err(|error| format!("unable to serialize check output: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn failure_page(record: &RunRecord, requested_offset: usize, limit: usize) -> BoundedPage {
    let window = page_window(record.failures.len(), requested_offset, limit);
    let desired_count = window.end.saturating_sub(window.offset);
    let mut count = 0;
    let mut value = failure_page_document(record, window.offset, count);
    let mut bytes = compact_json(&value).unwrap_or_default();

    while count < desired_count {
        let candidate = failure_page_document(record, window.offset, count + 1);
        let Ok(candidate_bytes) = compact_json(&candidate) else {
            break;
        };
        if candidate_bytes.len() > OUTPUT_BYTE_LIMIT {
            break;
        }
        count += 1;
        value = candidate;
        bytes = candidate_bytes;
    }

    let next = (window.offset + count < record.failures.len()).then_some(window.offset + count);
    value["next"] = next.map_or(Value::Null, |offset| json!(offset));
    bytes = compact_json(&value).unwrap_or(bytes);
    BoundedPage {
        value,
        bytes,
        offset: window.offset,
        next,
    }
}

fn failure_page_document(record: &RunRecord, offset: usize, count: usize) -> Value {
    let end = offset.saturating_add(count).min(record.failures.len());
    let failures = record.failures[offset..end]
        .iter()
        .map(failure_summary_json)
        .collect::<Vec<_>>();
    json!({
        "command": "check",
        "operation": "failures",
        "run": record.id,
        "total": record.failures.len(),
        "offset": offset,
        "failures": failures,
        "next": (end < record.failures.len()).then_some(end),
    })
}

pub fn failure_summary_json(failure: &FailureRecord) -> Value {
    let mut value = Map::new();
    value.insert("id".to_owned(), json!(failure.id));
    value.insert(
        "check".to_owned(),
        json!(truncate_utf8(&failure.check, 256).0),
    );
    value.insert(
        "code".to_owned(),
        json!(truncate_utf8(&failure.code, 128).0),
    );
    if let Some(subject) = &failure.subject {
        value.insert("subject".to_owned(), json!(truncate_utf8(subject, 256).0));
    }
    if let Some(location) = &failure.location {
        let mut location_value = Map::new();
        location_value.insert(
            "path".to_owned(),
            json!(truncate_utf8(&location.path, 512).0),
        );
        if let Some(line) = location.line {
            location_value.insert("line".to_owned(), json!(line));
        }
        if let Some(column) = location.column {
            location_value.insert("column".to_owned(), json!(column));
        }
        value.insert("location".to_owned(), Value::Object(location_value));
    }
    Value::Object(value)
}

pub fn list_page(
    ids: &[String],
    systems: &[String],
    notices: &[super::model::Notice],
    requested_offset: usize,
    limit: usize,
) -> Result<BoundedPage, String> {
    let window = page_window(ids.len(), requested_offset, limit);
    let desired_count = window.end.saturating_sub(window.offset);
    let mut count = 0;
    let mut value = list_page_document(ids, systems, notices, window.offset, count);
    let mut bytes = compact_json(&value)?;
    if bytes.len() > OUTPUT_BYTE_LIMIT {
        return Err("catalog metadata exceeds the 8 KiB response ceiling".to_owned());
    }

    while count < desired_count {
        // Human listing also preserves complete IDs; reserve room for paging
        // metadata and notices rather than truncating selector identities.
        if ids[window.offset + count].len() > 4096 {
            break;
        }
        let candidate = list_page_document(ids, systems, notices, window.offset, count + 1);
        let Ok(candidate_bytes) = compact_json(&candidate) else {
            break;
        };
        if candidate_bytes.len() > OUTPUT_BYTE_LIMIT {
            break;
        }
        count += 1;
        value = candidate;
        bytes = candidate_bytes;
    }

    if desired_count > 0 && count == 0 {
        return Err(format!(
            "catalog check ID at offset {} cannot fit a bounded page (4 KiB ID limit, 8 KiB response ceiling)",
            window.offset,
        ));
    }
    let next = (window.offset + count < ids.len()).then_some(window.offset + count);
    value["next"] = next.map_or(Value::Null, |offset| json!(offset));
    bytes = compact_json(&value).unwrap_or(bytes);
    Ok(BoundedPage {
        value,
        bytes,
        offset: window.offset,
        next,
    })
}

fn list_page_document(
    ids: &[String],
    systems: &[String],
    notices: &[super::model::Notice],
    offset: usize,
    count: usize,
) -> Value {
    let end = offset.saturating_add(count).min(ids.len());
    let notices_omitted = notices.len().saturating_sub(4);
    let systems_omitted = systems.len().saturating_sub(4);
    let notices = notices
        .iter()
        .take(4)
        .map(|notice| {
            json!({
                "code": notice.code,
                "message": truncate_utf8(&notice.message, 512).0,
            })
        })
        .collect::<Vec<_>>();
    let systems = systems
        .iter()
        .take(4)
        .map(|system| truncate_utf8(system, 128).0)
        .collect::<Vec<_>>();
    let mut document = json!({
        "command": "check",
        "operation": "list",
        "systems": systems,
        "total": ids.len(),
        "offset": offset,
        "checks": &ids[offset..end],
        "next": (end < ids.len()).then_some(end),
        "notices": notices,
    });
    if systems_omitted > 0 {
        document["systems_omitted"] = json!(systems_omitted);
    }
    if notices_omitted > 0 {
        document["notices_omitted"] = json!(notices_omitted);
    }
    document
}

pub fn run_summary_json(record: &RunRecord) -> BoundedPage {
    let desired_count = record.failures.len().min(super::model::FAILURE_PAGE_LIMIT);
    let mut count = 0;
    let mut value = run_summary_document(record, count);
    let mut bytes = compact_json(&value).unwrap_or_default();

    while count < desired_count {
        let candidate = run_summary_document(record, count + 1);
        let Ok(candidate_bytes) = compact_json(&candidate) else {
            break;
        };
        if candidate_bytes.len() > OUTPUT_BYTE_LIMIT {
            break;
        }
        count += 1;
        value = candidate;
        bytes = candidate_bytes;
    }

    let next = (count < record.failures.len()).then_some(count);
    value["next"] = next.map_or(Value::Null, |offset| json!(offset));
    bytes = compact_json(&value).unwrap_or(bytes);
    BoundedPage {
        value,
        bytes,
        offset: 0,
        next,
    }
}

fn run_summary_document(record: &RunRecord, failure_count: usize) -> Value {
    let mut counts = Map::new();
    for (outcome, count) in record.counts() {
        if count > 0 {
            counts.insert(outcome.to_owned(), json!(count));
        }
    }
    let failures = record
        .failures
        .iter()
        .take(failure_count)
        .map(failure_summary_json)
        .collect::<Vec<_>>();
    let mut value = json!({
        "command": "check",
        "run": record.id,
        "status": record.status.as_str(),
        "counts": Value::Object(counts),
        "total": record.failures.len(),
        "offset": 0,
        "failures": failures,
        "next": (failure_count < record.failures.len()).then_some(failure_count),
    });
    if record.selection.partial {
        let mut scope = Map::new();
        if !record.selection.selectors.is_empty() {
            scope.insert(
                "checks".to_owned(),
                json!(
                    record
                        .selection
                        .selectors
                        .iter()
                        .take(4)
                        .map(|selector| truncate_utf8(selector, 256).0)
                        .collect::<Vec<_>>()
                ),
            );
            if record.selection.selectors.len() > 4 {
                scope.insert(
                    "checks_omitted".to_owned(),
                    json!(record.selection.selectors.len() - 4),
                );
            }
        }
        if !record.selection.systems.is_empty() {
            scope.insert(
                "systems".to_owned(),
                json!(
                    record
                        .selection
                        .systems
                        .iter()
                        .take(4)
                        .map(|system| truncate_utf8(system, 128).0)
                        .collect::<Vec<_>>()
                ),
            );
            if record.selection.systems.len() > 4 {
                scope.insert(
                    "systems_omitted".to_owned(),
                    json!(record.selection.systems.len() - 4),
                );
            }
        }
        value["scope"] = Value::Object(scope);
    }
    if !record.notices.is_empty() {
        value["notices"] = json!(
            record
                .notices
                .iter()
                .take(4)
                .map(|notice| {
                    json!({
                        "code": notice.code,
                        "message": truncate_utf8(&notice.message, 512).0,
                    })
                })
                .collect::<Vec<_>>()
        );
    }
    value
}

pub fn run_summary_text(record: &RunRecord) -> String {
    let page = run_summary_json(record);
    let mut shown = page
        .value
        .get("failures")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    loop {
        let mut text = run_summary_text_with_count(record, shown);
        if human_output_fits(&text) || shown == 0 {
            if !text.ends_with('\n') {
                text.push('\n');
            }
            return text;
        }
        shown -= 1;
    }
}

fn run_summary_text_with_count(record: &RunRecord, shown: usize) -> String {
    let counts = record.counts();
    let mut output = format!(
        "{}  {} · {} failure record{}",
        status_label(record.status),
        count_summary(&counts),
        record.failures.len(),
        if record.failures.len() == 1 { "" } else { "s" }
    );
    if record.selection.partial {
        output.push_str("\npartial selection");
        if !record.selection.selectors.is_empty() {
            output.push_str(": ");
            output.push_str(
                &record
                    .selection
                    .selectors
                    .iter()
                    .take(4)
                    .map(|selector| safe_field(selector, 160))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            if record.selection.selectors.len() > 4 {
                output.push_str(&format!(
                    " · {} more selectors",
                    record.selection.selectors.len() - 4
                ));
            }
        }
        if !record.selection.systems.is_empty() {
            output.push_str(" · systems ");
            output.push_str(
                &record
                    .selection
                    .systems
                    .iter()
                    .take(4)
                    .map(|system| safe_field(system, 80))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            if record.selection.systems.len() > 4 {
                output.push_str(&format!(
                    " · {} more systems",
                    record.selection.systems.len() - 4
                ));
            }
        }
    }
    for notice in record.notices.iter().take(4) {
        output.push_str("\nnotice: ");
        output.push_str(&safe_field(&notice.message, 512));
    }
    for failure in record.failures.iter().take(shown) {
        output.push('\n');
        output.push_str(&safe_field(&failure.id, 32));
        output.push(' ');
        output.push_str(&safe_field(&failure.check, 256));
        output.push(' ');
        output.push_str(&safe_field(&failure.code, 128));
        if let Some(subject) = &failure.subject {
            output.push(' ');
            output.push_str(&safe_field(subject, 128));
        }
        if let Some(location) = &failure.location {
            output.push(' ');
            output.push_str(&safe_field(&location.path, 256));
            if let Some(line) = location.line {
                output.push(':');
                output.push_str(&line.to_string());
            }
        }
    }
    if shown < record.failures.len() {
        output.push_str(&format!(
            "\n{} failures omitted; continue with `bloomery check failures --run {} --offset {shown}`",
            record.failures.len() - shown,
            safe_field(&record.id, 128)
        ));
    }
    output.push_str(&format!("\nrun {}", safe_field(&record.id, 128)));
    if let Some(failure) = record.failures.first() {
        output.push_str(&format!(
            " · details: `bloomery check details {} --run {}`",
            safe_field(&failure.id, 32),
            safe_field(&record.id, 128)
        ));
    }
    output
}

fn count_summary(counts: &std::collections::BTreeMap<&'static str, usize>) -> String {
    ["passed", "failed", "blocked", "canceled", "not_run"]
        .into_iter()
        .filter_map(|name| counts.get(name).map(|count| format!("{count} {name}")))
        .collect::<Vec<_>>()
        .join(" · ")
}

fn status_label(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Passed => "PASS",
        RunStatus::Failed => "FAIL",
        RunStatus::Error => "ERROR",
        RunStatus::Interrupted => "INTERRUPTED",
    }
}

fn human_output_fits(text: &str) -> bool {
    crate::output::colorize_check(text, true).len() <= OUTPUT_BYTE_LIMIT
}

fn safe_field(value: &str, limit: usize) -> String {
    truncate_utf8(normalize_display_text(value).trim(), limit).0
}

pub struct DetailPage<'a> {
    pub run: &'a RunRecord,
    pub failure: &'a FailureRecord,
    pub outcome: Option<&'a CheckRecord>,
    pub records: &'a [String],
    pub requested_offset: usize,
    pub limit: usize,
}

pub fn details_page(page: DetailPage<'_>) -> BoundedPage {
    let window = page_window(page.records.len(), page.requested_offset, page.limit);
    let desired_count = window.end.saturating_sub(window.offset);
    let mut count = 0;
    let mut value = details_document(&page, window.offset, count);
    let mut bytes = compact_json(&value).unwrap_or_default();
    while count < desired_count {
        let candidate = details_document(&page, window.offset, count + 1);
        let Ok(candidate_bytes) = compact_json(&candidate) else {
            break;
        };
        if candidate_bytes.len() > OUTPUT_BYTE_LIMIT {
            break;
        }
        count += 1;
        value = candidate;
        bytes = candidate_bytes;
    }
    let next = (window.offset + count < page.records.len()).then_some(window.offset + count);
    value["next"] = next.map_or(Value::Null, |offset| json!(offset));
    bytes = compact_json(&value).unwrap_or(bytes);
    BoundedPage {
        value,
        bytes,
        offset: window.offset,
        next,
    }
}

pub fn failure_focused_offset(
    run: &RunRecord,
    failure: &FailureRecord,
    outcome: Option<&CheckRecord>,
    records: &[String],
    limit: usize,
) -> usize {
    let mut lower = 0;
    let mut upper = limit.min(records.len());
    while lower < upper {
        let count = lower + (upper - lower).div_ceil(2);
        let offset = records.len() - count;
        let page = details_page(DetailPage {
            run,
            failure,
            outcome,
            records,
            requested_offset: offset,
            limit: count,
        });
        let shown = page
            .value
            .get("records")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        if shown == count && page.next.is_none() {
            lower = count;
        } else {
            upper = count - 1;
        }
    }
    records.len() - lower
}

fn details_document(page: &DetailPage<'_>, offset: usize, count: usize) -> Value {
    let end = offset.saturating_add(count).min(page.records.len());
    let (check, check_truncated) = truncate_utf8(&page.failure.check, 512);
    let (code, code_truncated) = truncate_utf8(&page.failure.code, 128);
    let (message, message_truncated) = truncate_utf8(&page.failure.message, 1024);
    let records = page.records[offset..end]
        .iter()
        .map(|record| record.as_str())
        .collect::<Vec<_>>();
    let mut document = json!({
        "command": "check",
        "operation": "details",
        "run": page.run.id,
        "failure": page.failure.id,
        "check": check,
        "outcome": page
            .outcome
            .map_or("failed", |outcome| outcome.outcome.as_str()),
        "code": code,
        "message": message,
        "records": records,
        "offset": offset,
        "total": page.records.len(),
        "next": (end < page.records.len()).then_some(end),
        "metadata_truncated": check_truncated || code_truncated || message_truncated,
    });
    if let Some(location) = &page.failure.location {
        let mut location_value = Map::new();
        location_value.insert(
            "path".to_owned(),
            json!(truncate_utf8(&location.path, 512).0),
        );
        if let Some(line) = location.line {
            location_value.insert("line".to_owned(), json!(line));
        }
        if let Some(column) = location.column {
            location_value.insert("column".to_owned(), json!(column));
        }
        document["location"] = Value::Object(location_value);
    }
    document
}

pub fn details_text(page: &DetailPage<'_>, bounded: &BoundedPage) -> String {
    let available = bounded
        .value
        .get("records")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    for count in (0..=available).rev() {
        let mut output = format!(
            "run {} · {} · {} {}\ncheck: {}\n{}\n",
            safe_field(&page.run.id, 128),
            safe_field(&page.failure.id, 32),
            page.outcome
                .map_or("failed", |outcome| outcome.outcome.as_str()),
            safe_field(&page.failure.code, 128),
            safe_field(&page.failure.check, 512),
            safe_field(&page.failure.message, 1024),
        );
        if let Some(location) = &page.failure.location {
            output.push_str("location: ");
            output.push_str(&safe_field(&location.path, 512));
            if let Some(line) = location.line {
                output.push(':');
                output.push_str(&line.to_string());
            }
            output.push('\n');
        }
        let end = bounded.offset.saturating_add(count).min(page.records.len());
        for record in &page.records[bounded.offset..end] {
            output.push_str(record);
            output.push('\n');
        }
        let next = (end < page.records.len()).then_some(end);
        output.push_str(&format!(
            "offset {} · total {} · next {}\n",
            bounded.offset,
            page.records.len(),
            next.map_or_else(|| "none".to_owned(), |next| next.to_string())
        ));
        if human_output_fits(&output) {
            return output;
        }
    }
    String::new()
}

pub fn failures_text(record: &RunRecord, page: &BoundedPage) -> String {
    let failures = page
        .value
        .get("failures")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for count in (0..=failures.len()).rev() {
        let mut output = format!(
            "run {} · failures {} · offset {}\n",
            safe_field(&record.id, 128),
            record.failures.len(),
            page.offset
        );
        for failure in failures.iter().take(count) {
            let id = failure.get("id").and_then(Value::as_str).unwrap_or("");
            let check = failure.get("check").and_then(Value::as_str).unwrap_or("");
            let code = failure.get("code").and_then(Value::as_str).unwrap_or("");
            output.push_str(&format!(
                "{} {} {}\n",
                safe_field(id, 32),
                safe_field(check, 256),
                safe_field(code, 128)
            ));
        }
        let end = page.offset.saturating_add(count).min(record.failures.len());
        let next = (end < record.failures.len()).then_some(end);
        output.push_str(&format!(
            "total {} · offset {} · next {}\n",
            record.failures.len(),
            page.offset,
            next.map_or_else(|| "none".to_owned(), |next| next.to_string())
        ));
        if human_output_fits(&output) {
            return output;
        }
    }
    String::new()
}

pub fn list_text(ids: &[String], systems: &[String], page: &BoundedPage) -> String {
    let checks = page
        .value
        .get("checks")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let displayed_systems = systems
        .iter()
        .take(4)
        .map(|system| safe_field(system, 128))
        .collect::<Vec<_>>();
    let systems_omitted = systems.len().saturating_sub(displayed_systems.len());
    let notices = page
        .value
        .get("notices")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for count in (0..=checks.len()).rev() {
        let mut output = format!(
            "checks {} · offset {} · systems {}\n",
            ids.len(),
            page.offset,
            displayed_systems.join(", ")
        );
        if systems_omitted > 0 {
            output.push_str(&format!("{} additional systems omitted\n", systems_omitted));
        }
        for notice in &notices {
            if let Some(message) = notice.get("message").and_then(Value::as_str) {
                output.push_str("notice: ");
                output.push_str(&safe_field(message, 512));
                output.push('\n');
            }
        }
        for check in checks.iter().take(count) {
            if let Some(id) = check.as_str() {
                output.push_str(&safe_field(id, 4096));
                output.push('\n');
            }
        }
        let end = page.offset.saturating_add(count).min(ids.len());
        let next = (end < ids.len()).then_some(end);
        output.push_str(&format!(
            "total {} · offset {} · next {}\n",
            ids.len(),
            page.offset,
            next.map_or_else(|| "none".to_owned(), |next| next.to_string())
        ));
        if human_output_fits(&output) {
            return output;
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::{
        DetailPage, details_page, details_text, failure_page, failure_summary_json, failures_text,
        list_page, list_text, run_summary_json, run_summary_text,
    };
    use crate::check_command::model::{
        CheckRecord, FailureLocation, FailureRecord, OUTPUT_BYTE_LIMIT, Outcome, RunRecord,
        RunSelection, RunStatus,
    };
    use bloomery_test_macros::bloomery;

    fn record(failure_count: usize) -> RunRecord {
        let failures = (0..failure_count)
            .map(|index| FailureRecord {
                id: format!("f{}", index + 1),
                check: format!("static:traceability:{index}"),
                code: "MissingAutomatedTest".to_owned(),
                subject: Some(format!("CLI-CHECK-RUN-{index:03}")),
                location: Some(FailureLocation {
                    path: ".bloomery/specs/CLI/CHECK/requirements/RUN.toml".to_owned(),
                    line: Some(index + 1),
                    column: None,
                }),
                message: "a detailed diagnostic that is not printed in the summary".to_owned(),
                notes: vec!["full note available from details".to_owned()],
                log: None,
                nix_log: None,
                focus_tail: false,
            })
            .collect();
        RunRecord {
            id: "r123".to_owned(),
            root: "/tmp/workspace".to_owned(),
            started_at: 10,
            completed_at: 20,
            source_revision: None,
            source_dirty: None,
            status: if failure_count == 0 {
                RunStatus::Passed
            } else {
                RunStatus::Failed
            },
            selection: RunSelection {
                selectors: Vec::new(),
                systems: Vec::new(),
                selected_checks: vec!["static:structure".to_owned()],
                partial: false,
            },
            outcomes: vec![CheckRecord {
                id: "static:structure".to_owned(),
                outcome: if failure_count == 0 {
                    Outcome::Passed
                } else {
                    Outcome::Failed
                },
                blocked_by: None,
                logs: Vec::new(),
            }],
            failures,
            notices: Vec::new(),
        }
    }

    #[test]
    #[bloomery("CLI-CHECK-OUTPUT-001")]
    #[bloomery("CLI-CHECK-OUTPUT-002")]
    #[bloomery("CLI-CHECK-OUTPUT-008")]
    #[bloomery("CLI-CHECK-OUTPUT-009")]
    #[bloomery("CLI-CHECK-OUTPUT-010")]
    #[bloomery("CLI-INTERFACE-OUTPUT-004")]
    fn run_summary_is_compact_counts_only_and_omits_logs_and_success_records() {
        let passing = record(0);
        let summary = run_summary_json(&passing);
        assert_eq!(summary.value["status"], "passed");
        assert_eq!(summary.value["counts"]["passed"], 1);
        assert_eq!(summary.value["total"], 0);
        assert!(summary.value.get("checks").is_none());
        assert!(!String::from_utf8_lossy(&summary.bytes).contains("detailed diagnostic"));

        let failed = record(1);
        let summary = run_summary_json(&failed);
        assert_eq!(summary.value["failures"][0]["id"], "f1");
        assert!(summary.value["failures"][0].get("message").is_none());
        assert_eq!(summary.bytes.last(), Some(&b'\n'));
        assert!(!String::from_utf8_lossy(&summary.bytes).contains("\u{001b}"));
    }

    #[test]
    #[bloomery("CLI-CHECK-OUTPUT-003")]
    #[bloomery("CLI-CHECK-OUTPUT-004")]
    #[bloomery("CLI-CHECK-OUTPUT-005")]
    #[bloomery("CLI-CHECK-OUTPUT-006")]
    fn initial_summary_page_is_bounded_and_failures_have_run_local_ids() {
        let large = record(80);
        let summary = run_summary_json(&large);
        assert!(summary.value["failures"].as_array().unwrap().len() <= 20);
        assert!(summary.bytes.len() <= OUTPUT_BYTE_LIMIT);
        assert_eq!(
            summary.value["next"],
            summary.value["failures"].as_array().unwrap().len()
        );
        assert_eq!(summary.value["total"], 80);
        assert!(run_summary_text(&large).len() <= OUTPUT_BYTE_LIMIT);
    }

    #[test]
    #[bloomery("CLI-CHECK-SELECT-017")]
    #[bloomery("CLI-CHECK-SELECT-018")]
    #[bloomery("CLI-CHECK-SELECT-019")]
    fn catalog_pages_are_sorted_seekable_and_bounded() {
        let ids = (0..500)
            .map(|index| format!("nix:x86_64-linux:check-{index:03}"))
            .collect::<Vec<_>>();
        let page =
            list_page(&ids, &["x86_64-linux".to_owned()], &[], 10, 1000).expect("catalog page");
        assert_eq!(page.offset, 10);
        assert_eq!(page.value["total"], 500);
        assert!(page.bytes.len() <= OUTPUT_BYTE_LIMIT);
        assert_eq!(
            page.next,
            page.value["next"].as_u64().map(|value| value as usize)
        );
        assert_eq!(ids[0], "nix:x86_64-linux:check-000");
        let human = list_text(&ids, &["x86_64-linux".to_owned()], &page);
        assert!(human.len() <= OUTPUT_BYTE_LIMIT);
        assert!(human.contains("offset 10 · next"));
    }

    #[test]
    #[bloomery("CLI-CHECK-SELECT-021")]
    fn oversized_catalog_entries_are_explicit_errors_without_truncating_ids() {
        let ids = vec!["first".to_owned(), "x".repeat(9000)];
        let first = list_page(&ids, &[], &[], 0, 20).expect("preceding entry fits");
        assert_eq!(first.value["checks"], serde_json::json!(["first"]));
        assert_eq!(first.next, Some(1));
        assert!(list_page(&ids, &[], &[], 1, 20).is_err());
        let end = list_page(&ids, &[], &[], 2, 20).expect("end page");
        assert_eq!(end.next, None);
        // Escaping can make an ID exceed the response ceiling despite fitting
        // the raw ID limit. It must not produce a non-advancing continuation.
        assert!(list_page(&["\u{0001}".repeat(2000)], &[], &[], 0, 20).is_err());
    }

    #[test]
    #[bloomery("CLI-CHECK-DETAIL-010")]
    #[bloomery("CLI-CHECK-DETAIL-011")]
    #[bloomery("CLI-CHECK-DETAIL-016")]
    #[bloomery("CLI-CHECK-DETAIL-020")]
    fn detail_pages_are_bounded_seekable_and_make_progress_in_both_formats() {
        let run = record(1);
        let failure = &run.failures[0];
        let outcome = &run.outcomes[0];
        let records = (0..100)
            .map(|index| format!("line {index}: {}", "x".repeat(400)))
            .collect::<Vec<_>>();
        let first = details_page(DetailPage {
            run: &run,
            failure,
            outcome: Some(outcome),
            records: &records,
            requested_offset: 0,
            limit: 100,
        });
        assert!(first.bytes.len() <= OUTPUT_BYTE_LIMIT);
        assert!(!first.value["records"].as_array().unwrap().is_empty());
        assert!(first.next.is_some_and(|next| next > first.offset));

        let middle = details_page(DetailPage {
            run: &run,
            failure,
            outcome: Some(outcome),
            records: &records,
            requested_offset: 20,
            limit: 3,
        });
        assert_eq!(middle.offset, 20);
        assert_eq!(middle.next, Some(23));

        let human = details_text(
            &DetailPage {
                run: &run,
                failure,
                outcome: Some(outcome),
                records: &records,
                requested_offset: 0,
                limit: 100,
            },
            &first,
        );
        assert!(human.len() <= OUTPUT_BYTE_LIMIT);
        assert!(human.contains("offset 0 · total 100 · next"));

        let end = details_page(DetailPage {
            run: &run,
            failure,
            outcome: Some(outcome),
            records: &records,
            requested_offset: records.len() + 1,
            limit: 10,
        });
        assert!(end.value["records"].as_array().unwrap().is_empty());
        assert_eq!(end.next, None);
    }

    #[test]
    #[bloomery("CLI-CHECK-DETAIL-006")]
    #[bloomery("CLI-CHECK-DETAIL-011")]
    fn failure_pages_report_the_actual_offset_and_continuation() {
        let run = record(50);
        let first = failure_page(&run, 10, 5);
        assert_eq!(first.offset, 10);
        assert_eq!(first.value["failures"].as_array().unwrap().len(), 5);
        assert_eq!(first.next, Some(15));
        assert_eq!(first.value["failures"][0]["id"], "f11");
        assert!(first.bytes.len() <= OUTPUT_BYTE_LIMIT);
        assert!(failures_text(&run, &first).contains("offset 10 · next 15"));

        let second = failure_page(&run, 15, 5);
        assert_eq!(second.value["failures"][0]["id"], "f16");
    }

    #[test]
    #[bloomery("CLI-CHECK-OUTPUT-011")]
    #[bloomery("CLI-CHECK-OUTPUT-012")]
    fn blocked_outcomes_count_without_duplicating_the_causal_failure() {
        let mut run = record(1);
        run.outcomes.push(CheckRecord {
            id: "static:traceability".to_owned(),
            outcome: Outcome::Blocked,
            blocked_by: Some("f1".to_owned()),
            logs: Vec::new(),
        });
        let summary = run_summary_json(&run);
        assert_eq!(summary.value["counts"]["failed"], 1);
        assert_eq!(summary.value["counts"]["blocked"], 1);
        assert_eq!(summary.value["failures"].as_array().unwrap().len(), 1);
    }

    #[test]
    #[bloomery("CLI-CHECK-OUTPUT-010")]
    fn optional_failure_location_fields_are_omitted_when_unavailable() {
        let mut failure = record(1).failures.remove(0);
        failure.location = Some(FailureLocation {
            path: "failure.rs".to_owned(),
            line: None,
            column: None,
        });
        let location = failure_summary_json(&failure)["location"].clone();
        assert_eq!(location["path"], "failure.rs");
        assert!(location.get("line").is_none());
        assert!(location.get("column").is_none());
    }
}
