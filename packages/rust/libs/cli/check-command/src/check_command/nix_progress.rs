use super::progress::{DerivationEvent, ProgressEvent, ProgressSink};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

const BUILD_ACTIVITY: u64 = 105;
const SUBSTITUTE_ACTIVITY: u64 = 108;
const COPY_PATH_ACTIVITY: u64 = 100;
const BUILD_RESULT: u64 = 110;
const MAX_RECORD_BYTES: usize = 1 << 20;

/// Structured activity kinds that carry no derivation realization work. A
/// `start` record whose kind is absent from both this list and the recognized
/// work kinds makes collection unsupported instead of proving zero work.
const IGNORED_ACTIVITY_TYPES: &[u64] = &[0, 101, 102, 103, 104, 106, 107, 109, 110, 111, 112];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AssignedWork {
    Build,
    Substitute,
}

/// One structured activity frame extracted from Nix's `internal-json` log
/// format. Only the fields used for metric normalization are retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NixRecord {
    pub(super) action: String,
    pub(super) id: Option<u64>,
    pub(super) kind: Option<u64>,
    pub(super) fields: Vec<Value>,
    pub(super) payload: Option<Value>,
}

impl NixRecord {
    fn path(&self) -> Option<&str> {
        self.fields.first().and_then(Value::as_str)
    }
}

/// Incremental newline-framed parser for `@nix {...}` records. It tolerates
/// split frames, malformed JSON, unknown records, and oversized garbage.
#[derive(Debug, Default)]
pub(super) struct InternalJsonParser {
    buffer: Vec<u8>,
}

impl InternalJsonParser {
    /// Feed bytes and collect complete records. Returns `true` when a non-empty
    /// line could not be parsed, which leaves metric availability incomplete.
    pub(super) fn push(&mut self, bytes: &[u8], out: &mut Vec<NixRecord>) -> bool {
        self.buffer.extend_from_slice(bytes);
        if self.buffer.len() > MAX_RECORD_BYTES && !self.buffer.contains(&b'\n') {
            self.buffer.clear();
            return true;
        }
        let mut malformed = false;
        let mut start = 0;
        while let Some(offset) = self.buffer[start..].iter().position(|byte| *byte == b'\n') {
            let end = start + offset;
            let line = self.buffer[start..end].to_vec();
            start = end + 1;
            match parse_record(&line) {
                Some(record) => out.push(record),
                None if !line.iter().all(u8::is_ascii_whitespace) => malformed = true,
                None => {}
            }
        }
        if start > 0 {
            self.buffer.drain(..start);
        }
        malformed
    }

    /// A final record without a newline terminator was never parsed; its
    /// absence from the record list must not be read as proven zero work.
    pub(super) fn has_pending(&self) -> bool {
        !self.buffer.is_empty()
    }
}

fn parse_record(line: &[u8]) -> Option<NixRecord> {
    let line = std::str::from_utf8(line).ok()?;
    let payload = line.strip_prefix("@nix ")?;
    let value: Value = serde_json::from_str(payload).ok()?;
    let action = value.get("action")?.as_str()?.to_owned();
    let id = value.get("id").and_then(Value::as_u64);
    let kind = value.get("type").and_then(Value::as_u64);
    let fields = value
        .get("fields")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let payload = value.get("payload").cloned();
    Some(NixRecord {
        action,
        id,
        kind,
        fields,
        payload,
    })
}

#[derive(Clone)]
pub(super) struct NixProgressCollector {
    inner: Arc<Mutex<CollectorState>>,
}

struct CollectorState {
    sink: ProgressSink,
    parser: InternalJsonParser,
    output_to_drv: BTreeMap<String, String>,
    assigned: BTreeMap<String, AssignedWork>,
    /// Activity ID to canonical derivation path, so per-derivation completion
    /// results can be attributed without re-parsing prose.
    activity_work: BTreeMap<u64, String>,
    saw_activity: bool,
    incomplete: bool,
    incomplete_reported: bool,
}

impl NixProgressCollector {
    pub(super) fn new(sink: ProgressSink) -> Self {
        Self {
            inner: Arc::new(Mutex::new(CollectorState {
                sink,
                parser: InternalJsonParser::default(),
                output_to_drv: BTreeMap::new(),
                assigned: BTreeMap::new(),
                activity_work: BTreeMap::new(),
                saw_activity: false,
                incomplete: false,
                incomplete_reported: false,
            })),
        }
    }

    pub(super) fn is_active(&self) -> bool {
        self.inner
            .lock()
            .map(|state| state.sink.is_enabled())
            .unwrap_or(false)
    }

    /// Register structured output-path to derivation-path metadata so
    /// substitution activities can be attributed to derivations. Failure to
    /// obtain metadata leaves substitution metrics unavailable without failing
    /// the check.
    pub(super) fn set_output_metadata(&self, metadata: BTreeMap<String, String>) {
        if let Ok(mut state) = self.inner.lock() {
            state.output_to_drv = metadata;
        }
    }

    pub(super) fn consume(&self, bytes: &[u8]) {
        let Ok(mut state) = self.inner.lock() else {
            return;
        };
        if !state.sink.is_enabled() {
            return;
        }
        let mut records = Vec::new();
        if state.parser.push(bytes, &mut records) {
            mark_incomplete(&mut state);
        }
        for record in records {
            match record.action.as_str() {
                "start" => observe_start(&mut state, record),
                "result" => observe_result(&mut state, record),
                // Stop records alone do not establish success, and message or
                // phase records carry no work information.
                "stop" | "msg" | "setPhase" => {}
                _ => mark_incomplete(&mut state),
            }
        }
    }

    pub(super) fn finish_success(&self) {
        let Ok(mut state) = self.inner.lock() else {
            return;
        };
        finish(&mut state, Finish::Success);
    }

    pub(super) fn finish_failure(&self) {
        let Ok(mut state) = self.inner.lock() else {
            return;
        };
        finish(&mut state, Finish::Failure);
    }

    pub(super) fn finish_canceled(&self) {
        let Ok(mut state) = self.inner.lock() else {
            return;
        };
        finish(&mut state, Finish::Canceled);
    }
}

fn observe_start(state: &mut CollectorState, record: NixRecord) {
    match record.kind {
        Some(BUILD_ACTIVITY) => {
            let Some(path) = record.path().map(str::to_owned) else {
                mark_incomplete(state);
                return;
            };
            if !path.ends_with(".drv") {
                mark_incomplete(state);
                return;
            }
            state.saw_activity = true;
            if let Some(id) = record.id {
                state.activity_work.insert(id, path.clone());
            }
            // A build for a derivation that was previously assigned to
            // substitution is a cache fallback.
            if state.assigned.get(&path) == Some(&AssignedWork::Substitute) {
                state
                    .sink
                    .work(DerivationEvent::SubstituteFallback { drv: path.clone() });
            }
            state.assigned.insert(path.clone(), AssignedWork::Build);
            state.sink.work(DerivationEvent::BuildStarted { drv: path });
        }
        Some(SUBSTITUTE_ACTIVITY) | Some(COPY_PATH_ACTIVITY) => {
            let Some(path) = record.path().map(str::to_owned) else {
                mark_incomplete(state);
                return;
            };
            let Some(drv) = state.output_to_drv.get(&path).cloned() else {
                // Without structured metadata the substitution cannot be
                // attributed to a derivation; metrics stay unavailable.
                mark_incomplete(state);
                return;
            };
            state.saw_activity = true;
            if let Some(id) = record.id {
                state.activity_work.insert(id, drv.clone());
            }
            if state.assigned.get(&drv) == Some(&AssignedWork::Build) {
                return;
            }
            state.assigned.insert(drv.clone(), AssignedWork::Substitute);
            state.sink.work(DerivationEvent::SubstituteStarted {
                drv,
                outstanding: 1,
            });
        }
        Some(kind) if IGNORED_ACTIVITY_TYPES.contains(&kind) => {}
        _ => mark_incomplete(state),
    }
}

fn observe_result(state: &mut CollectorState, record: NixRecord) {
    if record.kind != Some(BUILD_RESULT) {
        return;
    }
    let Some(payload) = record.payload else {
        mark_incomplete(state);
        return;
    };
    let success = payload.get("success").and_then(Value::as_bool);
    let status = payload.get("status").and_then(Value::as_str);
    let (Some(true), Some(status)) = (success, status) else {
        // Failure evidence leaves unfinished work outstanding; it does not
        // establish success or repair incomplete collection.
        if success.is_none() || status.is_none() {
            mark_incomplete(state);
        }
        return;
    };
    let Some(drv) = result_derivation(state, &payload, record.id) else {
        mark_incomplete(state);
        return;
    };
    match status {
        "Built" => state.sink.work(DerivationEvent::BuildSucceeded { drv }),
        "Substituted" => state
            .sink
            .work(DerivationEvent::SubstituteSucceeded { drv }),
        "AlreadyValid" | "ResolvesToAlreadyValid" => {
            state.sink.work(DerivationEvent::StoreReused { drv })
        }
        _ => mark_incomplete(state),
    }
}

/// Resolve a `resBuildResult` payload to the canonical derivation path of the
/// successful build, substitution, or store reuse it describes.
fn result_derivation(state: &CollectorState, payload: &Value, id: Option<u64>) -> Option<String> {
    if let Some(drv) = payload_drv_path(payload) {
        return Some(drv);
    }
    if let Some(path) = payload.get("path").and_then(Value::as_str)
        && let Some(drv) = state.output_to_drv.get(path)
    {
        return Some(drv.clone());
    }
    id.and_then(|id| state.activity_work.get(&id).cloned())
}

/// A derived-path payload is an object only for build results; its `drvPath`
/// may itself be a nested derived path.
fn payload_drv_path(payload: &Value) -> Option<String> {
    fn leaf(value: &Value) -> Option<&str> {
        match value {
            Value::String(path) => Some(path),
            Value::Object(object) => object.get("drvPath").and_then(leaf),
            _ => None,
        }
    }
    match payload.get("path")? {
        Value::Object(_) => leaf(payload.get("path")?).map(str::to_owned),
        _ => None,
    }
}

fn mark_incomplete(state: &mut CollectorState) {
    state.incomplete = true;
    if !state.incomplete_reported {
        state.incomplete_reported = true;
        state.sink.emit(ProgressEvent::DerivationMetricsIncomplete);
    }
}

enum Finish {
    Success,
    Failure,
    Canceled,
}

fn finish(state: &mut CollectorState, outcome: Finish) {
    if !state.sink.is_enabled() {
        return;
    }
    if state.incomplete || state.parser.has_pending() {
        if !state.incomplete_reported {
            state.incomplete_reported = true;
            state.sink.emit(ProgressEvent::DerivationMetricsIncomplete);
        }
        finish_assigned(state, outcome);
        return;
    }
    if !state.saw_activity {
        // The request completed with complete structured output and no
        // realization activity, which proves all outputs were already in the
        // store. Derivation metrics are known to be zero.
        state.sink.emit(ProgressEvent::DerivationMetadataAvailable);
        return;
    }
    finish_assigned(state, outcome);
}

fn finish_assigned(state: &CollectorState, outcome: Finish) {
    match outcome {
        // A successful top-level request proves every derivation it actually
        // built or substituted finished successfully. Per-derivation results
        // already published during the run are deduplicated by the reducer.
        Finish::Success => {
            for (drv, work) in &state.assigned {
                match work {
                    AssignedWork::Build => state
                        .sink
                        .work(DerivationEvent::BuildSucceeded { drv: drv.clone() }),
                    AssignedWork::Substitute => state
                        .sink
                        .work(DerivationEvent::SubstituteSucceeded { drv: drv.clone() }),
                }
            }
        }
        // Failed and canceled work stays outstanding; no success is invented.
        Finish::Failure | Finish::Canceled => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{AssignedWork, InternalJsonParser, NixProgressCollector, NixRecord, parse_record};
    use crate::check_command::progress::{DerivationEvent, ProgressEvent, ProgressSink};
    use bloomery_test_macros::bloomery;
    use std::collections::BTreeMap;
    use std::sync::mpsc::{self, Receiver};

    fn sink() -> (ProgressSink, Receiver<ProgressEvent>) {
        let (sender, receiver) = mpsc::channel();
        (ProgressSink::new(sender, true), receiver)
    }

    fn frame(kind: u64, path: &str) -> String {
        format!("@nix {{\"action\":\"start\",\"id\":1,\"type\":{kind},\"fields\":[\"{path}\"]}}\n")
    }

    fn build_result(id: u64, drv: &str, status: &str) -> String {
        format!(
            "@nix {{\"action\":\"result\",\"id\":{id},\"type\":110,\"payload\":{{\"success\":true,\"status\":\"{status}\",\"builtOutputs\":{{}},\"path\":{{\"drvPath\":\"{drv}\",\"outputs\":[\"out\"]}}}}}}\n"
        )
    }

    fn substitute_result(id: u64, path: &str) -> String {
        format!(
            "@nix {{\"action\":\"result\",\"id\":{id},\"type\":110,\"payload\":{{\"success\":true,\"status\":\"Substituted\",\"path\":\"{path}\"}}}}\n"
        )
    }

    fn collected(receiver: &Receiver<ProgressEvent>) -> Vec<ProgressEvent> {
        let mut events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        events
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-022")]
    fn build_activities_normalize_to_derivation_events() {
        let (sink, receiver) = sink();
        let collector = NixProgressCollector::new(sink);
        collector.consume(frame(105, "/nix/store/abc-check.drv").as_bytes());
        assert_eq!(
            collected(&receiver),
            vec![ProgressEvent::Work(DerivationEvent::BuildStarted {
                drv: "/nix/store/abc-check.drv".to_owned(),
            })]
        );
        collector.finish_success();
        assert_eq!(
            collected(&receiver),
            vec![ProgressEvent::Work(DerivationEvent::BuildSucceeded {
                drv: "/nix/store/abc-check.drv".to_owned(),
            })]
        );
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-022")]
    fn substitution_activities_use_structured_output_metadata() {
        let (sink, receiver) = sink();
        let collector = NixProgressCollector::new(sink);
        collector.set_output_metadata(BTreeMap::from([(
            "/nix/store/abc-out".to_owned(),
            "/nix/store/abc-dep.drv".to_owned(),
        )]));
        collector.consume(frame(108, "/nix/store/abc-out").as_bytes());
        assert_eq!(
            collected(&receiver),
            vec![ProgressEvent::Work(DerivationEvent::SubstituteStarted {
                drv: "/nix/store/abc-dep.drv".to_owned(),
                outstanding: 1,
            })]
        );
        collector.finish_success();
        assert_eq!(
            collected(&receiver),
            vec![ProgressEvent::Work(DerivationEvent::SubstituteSucceeded {
                drv: "/nix/store/abc-dep.drv".to_owned(),
            })]
        );
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-020")]
    fn unattributable_substitutions_leave_metrics_unknown() {
        let (sink, receiver) = sink();
        let collector = NixProgressCollector::new(sink);
        collector.consume(frame(108, "/nix/store/unknown-out").as_bytes());
        assert_eq!(
            collected(&receiver),
            vec![ProgressEvent::DerivationMetricsIncomplete]
        );
        collector.finish_success();
        // The incomplete classification is latched; it is neither repeated nor
        // replaced with fabricated zero work.
        assert!(collected(&receiver).is_empty());
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-020")]
    #[bloomery("CLI-CHECK-PROGRESS-023")]
    fn malformed_and_unknown_records_leave_metrics_unknown() {
        let (sink, receiver) = sink();
        let collector = NixProgressCollector::new(sink);
        collector.consume(b"not a nix record\n");
        assert_eq!(
            collected(&receiver),
            vec![ProgressEvent::DerivationMetricsIncomplete]
        );
        collector.consume(b"@nix {not json}\n");
        collector.consume(b"@nix {\"action\":\"start\",\"type\":999,\"fields\":[\"x\"]}\n");
        collector.consume(b"@nix {\"action\":\"stop\",\"id\":1}\n");
        assert!(collected(&receiver).is_empty());
        collector.finish_success();
        assert!(collected(&receiver).is_empty());
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-020")]
    fn an_incomplete_final_record_keeps_metrics_unknown() {
        let (sink, receiver) = sink();
        let collector = NixProgressCollector::new(sink);
        collector
            .consume(b"@nix {\"action\":\"start\",\"type\":105,\"fields\":[\"/nix/store/a.drv\"]}");
        assert!(collected(&receiver).is_empty());
        collector.finish_success();
        assert_eq!(
            collected(&receiver),
            vec![ProgressEvent::DerivationMetricsIncomplete]
        );
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-013")]
    fn per_derivation_build_results_advance_success_during_the_request() {
        let (sink, receiver) = sink();
        let collector = NixProgressCollector::new(sink);
        collector.consume(frame(105, "/nix/store/a.drv").as_bytes());
        assert_eq!(
            collected(&receiver),
            vec![ProgressEvent::Work(DerivationEvent::BuildStarted {
                drv: "/nix/store/a.drv".to_owned(),
            })]
        );
        // A success-bearing result for the running activity is published
        // immediately, before the enclosing request exits.
        collector.consume(build_result(1, "/nix/store/a.drv", "Built").as_bytes());
        assert_eq!(
            collected(&receiver),
            vec![ProgressEvent::Work(DerivationEvent::BuildSucceeded {
                drv: "/nix/store/a.drv".to_owned(),
            })]
        );
        collector.finish_success();
        assert_eq!(
            collected(&receiver),
            vec![ProgressEvent::Work(DerivationEvent::BuildSucceeded {
                drv: "/nix/store/a.drv".to_owned(),
            })]
        );
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-013")]
    fn activity_stops_alone_do_not_establish_build_success() {
        let (sink, receiver) = sink();
        let collector = NixProgressCollector::new(sink);
        collector.consume(frame(105, "/nix/store/a.drv").as_bytes());
        let _ = collected(&receiver);
        // The protocol cannot establish success from a stop record, so the
        // derivation stays outstanding until request-level success.
        collector.consume(b"@nix {\"action\":\"stop\",\"id\":1}\n");
        assert!(collected(&receiver).is_empty());
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-015")]
    fn per_derivation_substitution_results_advance_cached_work() {
        let (sink, receiver) = sink();
        let collector = NixProgressCollector::new(sink);
        collector.set_output_metadata(BTreeMap::from([(
            "/nix/store/abc-out".to_owned(),
            "/nix/store/abc-dep.drv".to_owned(),
        )]));
        collector.consume(frame(108, "/nix/store/abc-out").as_bytes());
        let _ = collected(&receiver);
        collector.consume(substitute_result(1, "/nix/store/abc-out").as_bytes());
        assert_eq!(
            collected(&receiver),
            vec![ProgressEvent::Work(DerivationEvent::SubstituteSucceeded {
                drv: "/nix/store/abc-dep.drv".to_owned(),
            })]
        );
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-019")]
    fn per_derivation_store_reuse_results_exclude_finished_work() {
        let (sink, receiver) = sink();
        let collector = NixProgressCollector::new(sink);
        collector.consume(frame(105, "/nix/store/a.drv").as_bytes());
        let _ = collected(&receiver);
        collector.consume(build_result(1, "/nix/store/a.drv", "AlreadyValid").as_bytes());
        assert_eq!(
            collected(&receiver),
            vec![ProgressEvent::Work(DerivationEvent::StoreReused {
                drv: "/nix/store/a.drv".to_owned(),
            })]
        );
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-023")]
    fn split_and_truncated_frames_are_reassembled_without_losing_records() {
        let (sink, receiver) = sink();
        let collector = NixProgressCollector::new(sink);
        let record = frame(105, "/nix/store/split.drv");
        let (first, second) = record.split_at(10);
        collector.consume(first.as_bytes());
        assert!(collected(&receiver).is_empty());
        collector.consume(second.as_bytes());
        assert_eq!(
            collected(&receiver),
            vec![ProgressEvent::Work(DerivationEvent::BuildStarted {
                drv: "/nix/store/split.drv".to_owned(),
            })]
        );
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-023")]
    fn fallback_builds_reclassify_a_substituted_derivation() {
        let (sink, receiver) = sink();
        let collector = NixProgressCollector::new(sink);
        collector.set_output_metadata(BTreeMap::from([(
            "/nix/store/out".to_owned(),
            "/nix/store/dep.drv".to_owned(),
        )]));
        collector.consume(frame(108, "/nix/store/out").as_bytes());
        collector.consume(frame(105, "/nix/store/dep.drv").as_bytes());
        let events = collected(&receiver);
        assert!(
            events.contains(&ProgressEvent::Work(DerivationEvent::SubstituteFallback {
                drv: "/nix/store/dep.drv".to_owned(),
            }))
        );
        assert_eq!(
            events.last(),
            Some(&ProgressEvent::Work(DerivationEvent::BuildStarted {
                drv: "/nix/store/dep.drv".to_owned(),
            }))
        );
    }

    #[test]
    fn parser_extracts_only_well_formed_records() {
        let mut parser = InternalJsonParser::default();
        let mut records = Vec::new();
        let malformed = parser.push(
            b"@nix {\"action\":\"start\",\"id\":7,\"type\":105,\"fields\":[\"/nix/store/a.drv\"]}\n",
            &mut records,
        );
        assert!(!malformed);
        assert!(!parser.has_pending());
        assert_eq!(
            records,
            vec![NixRecord {
                action: "start".to_owned(),
                id: Some(7),
                kind: Some(105),
                fields: vec![serde_json::Value::String("/nix/store/a.drv".to_owned())],
                payload: None,
            }]
        );
        assert!(parse_record(b"plain log line").is_none());
        assert!(parse_record(b"@nix garbage").is_none());
    }

    #[test]
    fn parser_reports_malformed_and_pending_records() {
        let mut parser = InternalJsonParser::default();
        let mut records = Vec::new();
        let malformed = parser.push(b"plain log line\n", &mut records);
        assert!(malformed);
        assert!(records.is_empty());
        parser.push(b"@nix {\"action\":\"start\"}", &mut records);
        assert!(parser.has_pending());
    }

    #[test]
    fn assigned_work_is_equatable() {
        assert_eq!(AssignedWork::Build, AssignedWork::Build);
        assert_ne!(AssignedWork::Build, AssignedWork::Substitute);
    }
}
