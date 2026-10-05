use super::model::Outcome;
use crate::output::TransientLine;
use std::collections::BTreeMap;
use std::io::Write;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

/// A normalized progress event. Check finalization and derivation work share
/// one channel so a single consumer owns ordering and rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ProgressEvent {
    CheckFinalized {
        id: String,
        outcome: Outcome,
    },
    /// Structured Nix metadata became available even though the request did no
    /// derivation work (for example an all-store-hit request).
    DerivationMetadataAvailable,
    /// Structured information is missing, malformed, or insufficient, so the
    /// derivation metrics must stay unknown rather than displaying invented
    /// counts. This latches for the rest of the run.
    DerivationMetricsIncomplete,
    Work(DerivationEvent),
}

/// A normalized observation about one derivation's realization work. Not every
/// backend produces every variant; unavailable information is omitted.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(super) enum DerivationEvent {
    Discovered { drv: String },
    BuildStarted { drv: String },
    BuildSucceeded { drv: String },
    BuildFailed { drv: String },
    BuildCanceled { drv: String },
    SubstituteStarted { drv: String, outstanding: usize },
    SubstituteProgress { drv: String, outstanding: usize },
    SubstituteSucceeded { drv: String },
    SubstituteFallback { drv: String },
    StoreReused { drv: String },
}

/// Cloneable, thread-safe event sink used by the Nix backend and the scheduler.
/// A disabled sink drops events, which lets callers skip expensive progress
/// collection when live rendering is suppressed.
#[derive(Clone)]
pub(super) struct ProgressSink {
    sender: Arc<Mutex<mpsc::Sender<ProgressEvent>>>,
    enabled: bool,
}

impl ProgressSink {
    pub(super) fn new(sender: mpsc::Sender<ProgressEvent>, enabled: bool) -> Self {
        Self {
            sender: Arc::new(Mutex::new(sender)),
            enabled,
        }
    }

    pub(super) fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(super) fn emit(&self, event: ProgressEvent) {
        if !self.enabled {
            return;
        }
        // A disconnected consumer disables delivery; it must never abort work.
        if let Ok(sender) = self.sender.lock() {
            let _ = sender.send(event);
        }
    }

    pub(super) fn work(&self, event: DerivationEvent) {
        self.emit(ProgressEvent::Work(event));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DerivationWork {
    Build { succeeded: bool },
    Substitute { outstanding: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProgressSnapshot {
    pub(super) checks_complete: usize,
    pub(super) checks_total: usize,
    pub(super) built: Option<usize>,
    pub(super) to_build: Option<usize>,
    pub(super) cached: Option<usize>,
    pub(super) to_fetch: Option<usize>,
    pub(super) total: Option<usize>,
}

/// Pure run-scoped reducer. It owns exactly-once check completion and
/// deduplicated derivation work accounting. It never performs I/O.
pub(super) struct ProgressReducer {
    checks_total: usize,
    finalized: BTreeMap<String, Outcome>,
    derivations: BTreeMap<String, DerivationWork>,
    work_known: bool,
    work_incomplete: bool,
}

impl ProgressReducer {
    /// `checks_total` is the fixed, deduplicated, prerequisite-expanded
    /// selection count. Derivation metrics stay unknown until structured
    /// information arrives, except for static-only runs which are known to be
    /// zero.
    pub(super) fn new(checks_total: usize, has_nix_checks: bool) -> Self {
        Self {
            checks_total,
            finalized: BTreeMap::new(),
            derivations: BTreeMap::new(),
            work_known: !has_nix_checks,
            work_incomplete: false,
        }
    }

    pub(super) fn apply(&mut self, event: ProgressEvent) -> bool {
        let before = self.snapshot();
        match event {
            ProgressEvent::CheckFinalized { id, outcome } => {
                // A finalized check counts once. Re-finalization reconciles the
                // recorded outcome (for example a later blocked classification)
                // without changing the completion count.
                self.finalized.insert(id, outcome);
            }
            ProgressEvent::DerivationMetadataAvailable => {
                if !self.work_incomplete {
                    self.work_known = true;
                }
            }
            ProgressEvent::DerivationMetricsIncomplete => {
                // Unknown information can never be repaired into proven zero.
                self.work_incomplete = true;
                self.work_known = false;
            }
            ProgressEvent::Work(event) => {
                if !self.work_incomplete {
                    self.work_known = true;
                }
                self.apply_work(event);
            }
        }
        self.snapshot() != before
    }

    fn apply_work(&mut self, event: DerivationEvent) {
        match event {
            DerivationEvent::Discovered { drv } => {
                self.derivations
                    .entry(drv)
                    .or_insert(DerivationWork::Build { succeeded: false });
            }
            DerivationEvent::BuildStarted { drv } => {
                if self.derivations.get(&drv) != Some(&DerivationWork::Build { succeeded: true }) {
                    self.derivations
                        .insert(drv, DerivationWork::Build { succeeded: false });
                }
            }
            DerivationEvent::BuildSucceeded { drv } => {
                self.derivations
                    .insert(drv, DerivationWork::Build { succeeded: true });
            }
            DerivationEvent::BuildFailed { drv } | DerivationEvent::BuildCanceled { drv } => {
                // Successful evidence is never downgraded by a stale failure.
                if self.derivations.get(&drv) != Some(&DerivationWork::Build { succeeded: true }) {
                    self.derivations
                        .insert(drv, DerivationWork::Build { succeeded: false });
                }
            }
            DerivationEvent::SubstituteStarted { drv, outstanding } => {
                self.start_substitution(drv, outstanding);
            }
            DerivationEvent::SubstituteProgress { drv, outstanding } => {
                if let Some(DerivationWork::Substitute {
                    outstanding: current,
                }) = self.derivations.get_mut(&drv)
                    && *current > 0
                {
                    *current = outstanding;
                }
            }
            DerivationEvent::SubstituteSucceeded { drv } => {
                self.derivations
                    .insert(drv, DerivationWork::Substitute { outstanding: 0 });
            }
            DerivationEvent::SubstituteFallback { drv } => {
                if self.derivations.get(&drv) != Some(&DerivationWork::Build { succeeded: true }) {
                    self.derivations
                        .insert(drv, DerivationWork::Build { succeeded: false });
                }
            }
            DerivationEvent::StoreReused { drv } => {
                // Existing store outputs require no realization work.
                self.derivations.remove(&drv);
            }
        }
    }

    fn start_substitution(&mut self, drv: String, outstanding: usize) {
        match self.derivations.get(&drv) {
            // Completed or cached work is never downgraded by a duplicate.
            Some(DerivationWork::Build { succeeded: true })
            | Some(DerivationWork::Substitute { outstanding: 0 }) => {}
            _ => {
                self.derivations
                    .insert(drv, DerivationWork::Substitute { outstanding });
            }
        }
    }

    pub(super) fn snapshot(&self) -> ProgressSnapshot {
        let (built, to_build, cached, to_fetch, total) = if self.work_known {
            let mut built = 0;
            let mut to_build = 0;
            let mut cached = 0;
            let mut to_fetch = 0;
            for work in self.derivations.values() {
                match work {
                    DerivationWork::Build { succeeded: true } => built += 1,
                    DerivationWork::Build { succeeded: false } => to_build += 1,
                    DerivationWork::Substitute { outstanding: 0 } => cached += 1,
                    DerivationWork::Substitute { outstanding } => {
                        let _ = outstanding;
                        to_fetch += 1;
                    }
                }
            }
            let total = built + to_build + cached + to_fetch;
            (
                Some(built),
                Some(to_build),
                Some(cached),
                Some(to_fetch),
                Some(total),
            )
        } else {
            (None, None, None, None, None)
        };
        ProgressSnapshot {
            checks_complete: self.finalized.len().min(self.checks_total),
            checks_total: self.checks_total,
            built,
            to_build,
            cached,
            to_fetch,
            total,
        }
    }

    #[cfg(test)]
    fn final_outcome(&self, id: &str) -> Option<Outcome> {
        self.finalized.get(id).copied()
    }
}

/// Owns the reducer and the single transient terminal line for one run.
pub(super) struct ProgressReporter<W: Write> {
    reducer: ProgressReducer,
    line: TransientLine<W>,
    width: usize,
}

impl<W: Write> ProgressReporter<W> {
    pub(super) fn new(
        reducer: ProgressReducer,
        writer: W,
        enabled: bool,
        width: usize,
        styled: bool,
    ) -> Self {
        Self {
            reducer,
            line: TransientLine::new(writer, enabled, width, styled),
            width,
        }
    }

    pub(super) fn start(&mut self) {
        let text = format_progress_snapshot(&self.reducer.snapshot(), self.width);
        self.line.update(&text);
    }

    pub(super) fn apply(&mut self, event: ProgressEvent) {
        if self.reducer.apply(event) {
            let text = format_progress_snapshot(&self.reducer.snapshot(), self.width);
            self.line.update(&text);
        }
    }

    pub(super) fn is_enabled(&self) -> bool {
        self.line.is_enabled()
    }

    /// Final derivation work metrics for the completion summary. The transient
    /// pending counters (`to build`, `to fetch`) are dropped and check outcome
    /// counts already appear in the summary, so this is a compact final work
    /// status rather than another live progress frame.
    pub(super) fn final_metrics(&self) -> String {
        format_work_metrics(&self.reducer.snapshot(), self.width)
    }

    /// Clear the transient line before any final summary, error, or
    /// interruption report.
    pub(super) fn finish(&mut self) {
        self.line.clear();
    }
}

fn metric(value: Option<usize>) -> String {
    value.map_or_else(|| "?".to_owned(), |value| value.to_string())
}

/// Render a snapshot into one line, choosing shorter label forms before the
/// terminal width is exceeded so a replacement never wraps or accumulates.
fn format_progress_snapshot(snapshot: &ProgressSnapshot, width: usize) -> String {
    let complete = snapshot.checks_complete;
    let total = snapshot.checks_total;
    let built = metric(snapshot.built);
    let to_build = metric(snapshot.to_build);
    let cached = metric(snapshot.cached);
    let to_fetch = metric(snapshot.to_fetch);
    let known = metric(snapshot.total);
    let candidates = [
        format!(
            "checks {complete}/{total} complete \u{b7} built {built} \u{b7} to build {to_build} \u{b7} cached {cached} \u{b7} to fetch {to_fetch} \u{b7} known total {known}"
        ),
        format!(
            "checks {complete}/{total} \u{b7} built {built} \u{b7} build {to_build} \u{b7} cached {cached} \u{b7} fetch {to_fetch} \u{b7} total {known}"
        ),
        format!(
            "checks {complete}/{total} \u{b7} b{built} \u{b7} tb{to_build} \u{b7} c{cached} \u{b7} tf{to_fetch} \u{b7} T{known}"
        ),
        format!("{complete}/{total} b{built} tb{to_build} c{cached} tf{to_fetch} T{known}"),
        format!("checks {complete}/{total}"),
        format!("{complete}/{total}"),
        format!("{complete}"),
    ];
    for candidate in &candidates {
        if candidate.chars().count() <= width.max(1) {
            return candidate.clone();
        }
    }
    candidates.last().cloned().unwrap_or_default()
}

/// Completed derivation work from a snapshot for the final status line. Pending
/// build and substitution counters are transient-only, so only successfully
/// built, substituted, and the known total are reported.
fn format_work_metrics(snapshot: &ProgressSnapshot, width: usize) -> String {
    let built = metric(snapshot.built);
    let cached = metric(snapshot.cached);
    let known = metric(snapshot.total);
    let candidates = [
        format!("built {built} \u{b7} cached {cached} \u{b7} total {known}"),
        format!("built {built} \u{b7} cached {cached} \u{b7} T{known}"),
        format!("b{built} \u{b7} c{cached} \u{b7} T{known}"),
        format!("b{built} c{cached} T{known}"),
    ];
    for candidate in &candidates {
        if candidate.chars().count() <= width.max(1) {
            return candidate.clone();
        }
    }
    candidates.last().cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        DerivationEvent, ProgressEvent, ProgressReducer, ProgressReporter, ProgressSink,
        ProgressSnapshot as Snapshot, format_progress_snapshot,
    };
    use crate::check_command::model::Outcome;
    use std::sync::mpsc;

    fn reducer(checks: usize) -> ProgressReducer {
        ProgressReducer::new(checks, true)
    }

    fn derivation(reducer: &mut ProgressReducer, event: DerivationEvent) {
        reducer.apply(ProgressEvent::Work(event));
    }

    fn check(reducer: &mut ProgressReducer, id: &str, outcome: Outcome) {
        reducer.apply(ProgressEvent::CheckFinalized {
            id: id.to_owned(),
            outcome,
        });
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-008"))]
    fn the_denominator_is_fixed_and_includes_prerequisites() {
        let mut reducer = ProgressReducer::new(3, false);
        assert_eq!(reducer.snapshot().checks_total, 3);
        check(&mut reducer, "static:structure", Outcome::Passed);
        derivation(
            &mut reducer,
            DerivationEvent::BuildSucceeded {
                drv: "/nix/store/a.drv".to_owned(),
            },
        );
        assert_eq!(reducer.snapshot().checks_total, 3);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-009"))]
    fn every_terminal_outcome_advances_completion_exactly_once() {
        let mut reducer = reducer(6);
        assert_eq!(reducer.snapshot().checks_complete, 0);
        for (index, outcome) in [
            Outcome::Passed,
            Outcome::Failed,
            Outcome::Blocked,
            Outcome::Canceled,
            Outcome::NotRun,
            Outcome::Passed,
        ]
        .into_iter()
        .enumerate()
        {
            check(&mut reducer, &format!("check-{index}"), outcome);
        }
        assert_eq!(reducer.snapshot().checks_complete, 6);
        // Duplicate finalization does not double count.
        check(&mut reducer, "check-0", Outcome::Failed);
        assert_eq!(reducer.snapshot().checks_complete, 6);
        assert_eq!(reducer.final_outcome("check-0"), Some(Outcome::Failed));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-010"))]
    fn queued_or_running_checks_remain_incomplete() {
        let reducer = reducer(4);
        assert_eq!(reducer.snapshot().checks_complete, 0);
        // Only explicit finalization advances the counter.
        let mut reducer = reducer;
        check(&mut reducer, "queued", Outcome::NotRun);
        assert_eq!(reducer.snapshot().checks_complete, 1);
        assert_eq!(reducer.snapshot().checks_total, 4);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-008"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-009"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-010"))]
    fn the_formatter_gate_is_a_fixed_completion_unit() {
        // The caller includes format:workspace in the fixed denominator.
        let mut reducer = ProgressReducer::new(3, false);
        assert_eq!(reducer.snapshot().checks_total, 3);
        assert_eq!(reducer.snapshot().checks_complete, 0);
        check(&mut reducer, "format:workspace", Outcome::Passed);
        assert_eq!(reducer.snapshot().checks_complete, 1);
        check(&mut reducer, "static:structure", Outcome::Passed);
        check(&mut reducer, "static:traceability", Outcome::Passed);
        assert_eq!(reducer.snapshot().checks_complete, 3);
        assert_eq!(
            reducer.final_outcome("format:workspace"),
            Some(Outcome::Passed)
        );
        // Re-finalizing the formatter gate never double counts.
        check(&mut reducer, "format:workspace", Outcome::Failed);
        assert_eq!(reducer.snapshot().checks_complete, 3);
        assert_eq!(reducer.snapshot().checks_total, 3);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-011"))]
    fn alias_checks_retain_separate_completion_units() {
        let mut reducer = reducer(2);
        derivation(
            &mut reducer,
            DerivationEvent::BuildStarted {
                drv: "/nix/store/shared.drv".to_owned(),
            },
        );
        check(&mut reducer, "nix:x86_64-linux:alias-a", Outcome::Passed);
        check(&mut reducer, "nix:x86_64-linux:alias-b", Outcome::Passed);
        assert_eq!(reducer.snapshot().checks_complete, 2);
        // One shared derivation contributes one build work unit.
        assert_eq!(reducer.snapshot().to_build, Some(1));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-012"))]
    fn shared_dependencies_are_deduplicated_by_canonical_derivation_path() {
        let mut reducer = reducer(2);
        for drv in [
            "/nix/store/shared.drv",
            "/nix/store/shared.drv",
            "/nix/store/shared.drv",
        ] {
            derivation(
                &mut reducer,
                DerivationEvent::BuildStarted {
                    drv: drv.to_owned(),
                },
            );
        }
        assert_eq!(
            reducer.snapshot(),
            Snapshot {
                checks_complete: 0,
                checks_total: 2,
                built: Some(0),
                to_build: Some(1),
                cached: Some(0),
                to_fetch: Some(0),
                total: Some(1),
            }
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-013"))]
    fn built_counts_require_successful_completion() {
        let mut reducer = reducer(1);
        derivation(
            &mut reducer,
            DerivationEvent::BuildStarted {
                drv: "/nix/store/a.drv".to_owned(),
            },
        );
        derivation(
            &mut reducer,
            DerivationEvent::BuildFailed {
                drv: "/nix/store/a.drv".to_owned(),
            },
        );
        assert_eq!(reducer.snapshot().built, Some(0));
        assert_eq!(reducer.snapshot().to_build, Some(1));
        derivation(
            &mut reducer,
            DerivationEvent::BuildSucceeded {
                drv: "/nix/store/a.drv".to_owned(),
            },
        );
        assert_eq!(reducer.snapshot().built, Some(1));
        assert_eq!(reducer.snapshot().to_build, Some(0));
        // A stale failure does not erase success.
        derivation(
            &mut reducer,
            DerivationEvent::BuildFailed {
                drv: "/nix/store/a.drv".to_owned(),
            },
        );
        assert_eq!(reducer.snapshot().built, Some(1));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-014"))]
    fn unfinished_builds_remain_outstanding() {
        let mut reducer = reducer(3);
        for drv in [
            "/nix/store/active.drv",
            "/nix/store/failed.drv",
            "/nix/store/canceled.drv",
        ] {
            derivation(
                &mut reducer,
                DerivationEvent::BuildStarted {
                    drv: drv.to_owned(),
                },
            );
            derivation(
                &mut reducer,
                DerivationEvent::Discovered {
                    drv: drv.to_owned(),
                },
            );
        }
        derivation(
            &mut reducer,
            DerivationEvent::BuildFailed {
                drv: "/nix/store/failed.drv".to_owned(),
            },
        );
        derivation(
            &mut reducer,
            DerivationEvent::BuildCanceled {
                drv: "/nix/store/canceled.drv".to_owned(),
            },
        );
        assert_eq!(reducer.snapshot().to_build, Some(3));
        assert_eq!(reducer.snapshot().built, Some(0));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-015"))]
    fn cached_counts_require_every_required_output() {
        let mut reducer = reducer(1);
        derivation(
            &mut reducer,
            DerivationEvent::SubstituteStarted {
                drv: "/nix/store/multi.drv".to_owned(),
                outstanding: 2,
            },
        );
        derivation(
            &mut reducer,
            DerivationEvent::SubstituteProgress {
                drv: "/nix/store/multi.drv".to_owned(),
                outstanding: 1,
            },
        );
        assert_eq!(reducer.snapshot().cached, Some(0));
        assert_eq!(reducer.snapshot().to_fetch, Some(1));
        derivation(
            &mut reducer,
            DerivationEvent::SubstituteSucceeded {
                drv: "/nix/store/multi.drv".to_owned(),
            },
        );
        assert_eq!(reducer.snapshot().cached, Some(1));
        assert_eq!(reducer.snapshot().to_fetch, Some(0));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-016"))]
    fn outstanding_substitutions_remain_visible_once_per_derivation() {
        let mut reducer = reducer(1);
        for _ in 0..3 {
            derivation(
                &mut reducer,
                DerivationEvent::SubstituteStarted {
                    drv: "/nix/store/pending.drv".to_owned(),
                    outstanding: 4,
                },
            );
        }
        derivation(
            &mut reducer,
            DerivationEvent::SubstituteProgress {
                drv: "/nix/store/pending.drv".to_owned(),
                outstanding: 2,
            },
        );
        assert_eq!(reducer.snapshot().to_fetch, Some(1));
        assert_eq!(reducer.snapshot().total, Some(1));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-017"))]
    fn substitution_fallback_reclassifies_without_duplicating() {
        let mut reducer = reducer(1);
        derivation(
            &mut reducer,
            DerivationEvent::SubstituteStarted {
                drv: "/nix/store/fallback.drv".to_owned(),
                outstanding: 1,
            },
        );
        derivation(
            &mut reducer,
            DerivationEvent::SubstituteFallback {
                drv: "/nix/store/fallback.drv".to_owned(),
            },
        );
        derivation(
            &mut reducer,
            DerivationEvent::BuildStarted {
                drv: "/nix/store/fallback.drv".to_owned(),
            },
        );
        assert_eq!(reducer.snapshot().to_fetch, Some(0));
        assert_eq!(reducer.snapshot().cached, Some(0));
        assert_eq!(reducer.snapshot().to_build, Some(1));
        assert_eq!(reducer.snapshot().total, Some(1));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-018"))]
    fn known_total_partitions_discovered_work_and_grows_incrementally() {
        let mut reducer = reducer(1);
        derivation(
            &mut reducer,
            DerivationEvent::BuildSucceeded {
                drv: "/nix/store/built.drv".to_owned(),
            },
        );
        derivation(
            &mut reducer,
            DerivationEvent::BuildStarted {
                drv: "/nix/store/active.drv".to_owned(),
            },
        );
        derivation(
            &mut reducer,
            DerivationEvent::SubstituteSucceeded {
                drv: "/nix/store/cached.drv".to_owned(),
            },
        );
        derivation(
            &mut reducer,
            DerivationEvent::SubstituteStarted {
                drv: "/nix/store/fetch.drv".to_owned(),
                outstanding: 1,
            },
        );
        let snapshot = reducer.snapshot();
        assert_eq!(snapshot.total, Some(4));
        assert_eq!(
            snapshot.built.unwrap()
                + snapshot.to_build.unwrap()
                + snapshot.cached.unwrap()
                + snapshot.to_fetch.unwrap(),
            4
        );
        // Newly discovered work grows the known total.
        derivation(
            &mut reducer,
            DerivationEvent::Discovered {
                drv: "/nix/store/late.drv".to_owned(),
            },
        );
        assert_eq!(reducer.snapshot().total, Some(5));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-019"))]
    fn existing_store_reuse_is_excluded_from_work_counts() {
        let mut reducer = reducer(1);
        derivation(
            &mut reducer,
            DerivationEvent::Discovered {
                drv: "/nix/store/reused.drv".to_owned(),
            },
        );
        assert_eq!(reducer.snapshot().total, Some(1));
        derivation(
            &mut reducer,
            DerivationEvent::StoreReused {
                drv: "/nix/store/reused.drv".to_owned(),
            },
        );
        let snapshot = reducer.snapshot();
        assert_eq!(snapshot.total, Some(0));
        assert_eq!(snapshot.built, Some(0));
        assert_eq!(snapshot.cached, Some(0));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-020"))]
    fn unavailable_metrics_stay_unknown_until_structured_information_arrives() {
        let mut reducer = ProgressReducer::new(1, true);
        let snapshot = reducer.snapshot();
        assert_eq!(snapshot.built, None);
        assert_eq!(snapshot.total, None);
        assert_eq!(
            format_progress_snapshot(&snapshot, 200),
            "checks 0/1 complete \u{b7} built ? \u{b7} to build ? \u{b7} cached ? \u{b7} to fetch ? \u{b7} known total ?"
        );

        // An all-store-hit request publishes metadata with no derivation work.
        reducer.apply(ProgressEvent::DerivationMetadataAvailable);
        let snapshot = reducer.snapshot();
        assert_eq!(snapshot.built, Some(0));
        assert_eq!(snapshot.total, Some(0));
        assert_eq!(
            format_progress_snapshot(&snapshot, 200),
            "checks 0/1 complete \u{b7} built 0 \u{b7} to build 0 \u{b7} cached 0 \u{b7} to fetch 0 \u{b7} known total 0"
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-021"))]
    fn static_only_runs_report_known_zero_derivation_work() {
        let reducer = ProgressReducer::new(2, false);
        let snapshot = reducer.snapshot();
        assert_eq!(snapshot.built, Some(0));
        assert_eq!(snapshot.to_build, Some(0));
        assert_eq!(snapshot.cached, Some(0));
        assert_eq!(snapshot.to_fetch, Some(0));
        assert_eq!(snapshot.total, Some(0));
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-020"))]
    fn incomplete_structured_information_latches_metrics_unknown() {
        let mut reducer = ProgressReducer::new(1, true);
        reducer.apply(ProgressEvent::DerivationMetadataAvailable);
        assert_eq!(reducer.snapshot().total, Some(0));
        // Incompleteness overrides proven zero and cannot be repaired by later
        // optimistic observations during the same run.
        reducer.apply(ProgressEvent::DerivationMetricsIncomplete);
        assert_eq!(reducer.snapshot().built, None);
        assert_eq!(reducer.snapshot().total, None);
        reducer.apply(ProgressEvent::Work(DerivationEvent::BuildStarted {
            drv: "/nix/store/late.drv".to_owned(),
        }));
        reducer.apply(ProgressEvent::DerivationMetadataAvailable);
        assert_eq!(reducer.snapshot().built, None);
        assert_eq!(reducer.snapshot().total, None);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-PROGRESS-024"))]
    fn the_initial_snapshot_renders_once_and_unchanged_snapshots_do_not() {
        let mut output = Vec::new();
        {
            let mut reporter =
                ProgressReporter::new(ProgressReducer::new(2, true), &mut output, true, 200, false);
            reporter.start();
            // Re-rendering the unchanged initial tuple writes nothing.
            reporter.start();
            reporter.apply(ProgressEvent::CheckFinalized {
                id: "static:structure".to_owned(),
                outcome: Outcome::Passed,
            });
        }
        let text = String::from_utf8(output).expect("UTF-8");
        assert_eq!(text.matches("checks 0/2").count(), 1);
        assert_eq!(text.matches("checks 1/2").count(), 1);
    }

    #[test]
    fn snapshot_formatting_prefers_full_labels_and_shrinks_before_wrapping() {
        let snapshot = Snapshot {
            checks_complete: 3,
            checks_total: 12,
            built: Some(8),
            to_build: Some(4),
            cached: Some(16),
            to_fetch: Some(2),
            total: Some(30),
        };
        let full = format_progress_snapshot(&snapshot, 200);
        assert_eq!(
            full,
            "checks 3/12 complete \u{b7} built 8 \u{b7} to build 4 \u{b7} cached 16 \u{b7} to fetch 2 \u{b7} known total 30"
        );
        for width in [79, 60, 40, 20, 6, 3, 1] {
            let rendered = format_progress_snapshot(&snapshot, width);
            assert!(
                rendered.chars().count() <= width.max(1),
                "width {width} produced {rendered:?}"
            );
        }
        // A very wide terminal keeps the complete labels.
        assert!(format_progress_snapshot(&snapshot, 200).contains("known total 30"));
    }

    #[test]
    fn progress_sink_delivers_events_and_tolerates_a_dropped_consumer() {
        let (sender, receiver) = mpsc::channel();
        let sink = ProgressSink::new(sender, true);
        sink.work(DerivationEvent::BuildStarted {
            drv: "/nix/store/a.drv".to_owned(),
        });
        assert_eq!(
            receiver.recv().expect("event"),
            ProgressEvent::Work(DerivationEvent::BuildStarted {
                drv: "/nix/store/a.drv".to_owned(),
            })
        );
        drop(receiver);
        // Emitting after the consumer exits must not panic.
        sink.work(DerivationEvent::BuildFailed {
            drv: "/nix/store/a.drv".to_owned(),
        });

        let (sender, receiver) = mpsc::channel();
        let disabled = ProgressSink::new(sender, false);
        assert!(!disabled.is_enabled());
        disabled.work(DerivationEvent::BuildStarted {
            drv: "/nix/store/b.drv".to_owned(),
        });
        assert!(receiver.try_recv().is_err());
    }
}
