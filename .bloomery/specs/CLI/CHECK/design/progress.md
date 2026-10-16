# Terminal progress

## Eligibility

Execution of `bloomery check` displays live progress only in human mode when
stderr is a terminal, `TERM` is not `dumb`, and no CI marker is active. A non-empty
`CI` value is active unless it is `0` or `false` (case-insensitive). Non-empty
`GITHUB_ACTIONS`, `GITLAB_CI`, `BUILDKITE`, `TF_BUILD`, or `JENKINS_URL` also marks
CI, even when a pseudo-terminal is allocated. No progress override flag is
required. `NO_COLOR` disables styling, not progress.

Redirected stderr, JSON mode, CI, and dumb terminals receive no progress text or
cursor-control bytes on either stream. Eligibility depends on the progress
stream, not stdin or stdout: redirecting stdout alone preserves progress on a
user's terminal. `check list`, `check failures`, and `check details` do not emit
live progress.

## Check metrics

After selection validation and run allocation, initialize `checks complete` to
zero and `checks total` to the deduplicated selected check count, including
implicitly selected prerequisites, plus the `format:workspace` gate. The
denominator stays fixed throughout the run. The formatter gate, static checks,
and each selected Nix attribute count independently; aliases
retain separate check identities.

A check or formatter gate becomes complete once it has a final outcome: passed,
failed, blocked, canceled, or not_run. Queued work is not complete merely because
its provisional outcome is not_run. Update completion when outcomes become final
during execution, not only after all workers join. Count each outcome once.
Finalizing skipped or canceled work can bring the counter to the denominator
without implying success;
the final summary owns the pass/fail result.

## Derivation metrics

Nix dependency work is separate from the check counter. Aggregate selected
derivations and dependency work observed during the batched realization across
selected systems. Exclude transitive dependencies that Nix never needs to
realize because a parent output is already valid.
Deduplicate by canonical derivation store path, not check ID, activity ID,
display name, output count, or subprocess. Do not include unrelated
Nix daemon work. A derivation whose required outputs are already valid in the
local store is cached work even though Nix performs no download for it.

| Label | Meaning |
| --- | --- |
| `built` | Known derivations successfully built during this run, locally or remotely |
| `to build` | Known derivations assigned to building without successful build completion, including active, failed, and canceled work |
| `cached` | Known derivations whose required outputs are already valid in the local store or whose required missing outputs were successfully substituted during this run |
| `to fetch` | Known derivations assigned to substitution with required missing outputs still outstanding |
| `total` | Distinct known derivations in the selected realization, including derivations already satisfied by the local store |

A derivation with several required outputs contributes once. A partial download
is not a cached derivation. When substitution falls back to building, reclassify
that derivation rather than counting it in both categories. For fully known
metrics, `total = built + to build + cached + to fetch`. Derivations satisfied
by existing store outputs count once as cached, so a fast run that reuses prior
Nix builds reports that reuse in its final metrics.
Failures and cancellation do not increment successful build or cache counts and
do not erase known unfinished work.

Totals describe known realization work. Label the total `known total`; it grows
when Nix reports dependency work. Initialize it with distinct selected
derivations, classifying local-store hits as cached and other selected
derivations as pending. The full evaluation dependency graph does not establish
that Nix will realize every dependency. On successful completion with complete
metrics, `built + cached = total`. Display `?`
only while a metric cannot yet be determined from available structured
information; lack of a build activity alone does not make a known plan
unavailable. Static-only runs display zeros. A request that succeeds entirely
from existing store outputs can finish with zero build and substitution work,
nonzero cached work, and a completed check.

## Event collection

The Nix backend consumes structured evaluation work plans, local store validity,
and Nix activity during realization and sends normalized metric events to a
run-scoped aggregator. Publish each plan's distinct build and substitution
assignments as soon as they are available, then reconcile them with activity and
completion evidence as the run continues. Include derivations whose required
outputs are already valid in the local store and classify them as cached.
Resolve required output paths for selected checks before checking local
validity, including paths omitted from recursive derivation metadata. Probe
selected outputs only; progress collection does not query local validity for
the full dependency graph. Warm runs load metadata only for selected
derivations. Load recursive dependency metadata only when failure attribution
or observed dependency substitutions require it, after Nix realization has
started.
Consume per-derivation completion evidence as it arrives; an activity start or
stop alone does not establish success, and work whose success cannot be proven
remains outstanding. Resolve output substitution identities to derivation paths
using structured Nix metadata; store-path download counts are not derivation
counts. Do not scrape localized human diagnostics, forward raw Nix events to the
terminal, or sum aggregate totals from overlapping realization scopes.
Completed derivation work may be reconciled from the post-realization outcome
validity result when structured completion evidence is unavailable.

Execution supplies final-outcome events for selected checks and the formatter
gate to the same aggregator. Observe selected output validity periodically
alongside Nix realization so each completed check advances the counter before
the batch finishes, including separate check aliases. Validate all required
outputs before publishing a successful check outcome. One serialized renderer owns terminal writes;
workers do not render independently.
Collect progress without changing check selection, admission, Nix configuration,
realization ownership, or cancellation behavior. Nix remains the dependency
scheduler. Drain subprocess output continuously and retain diagnostic and builder
logs for details retrieval without streaming them as progress. A retained-log
write failure is an operational error; malformed progress information is
best-effort and cannot change check results.

Malformed, unsupported, or insufficient activity information leaves affected
metrics explicitly unavailable. Progress parsing is best-effort and cannot turn a
successful check into failure or mask an invocation failure. Missing progress
information does not mean no work is needed. Inject terminal/environment facts,
normalized Nix events, scheduler outcomes, and the output sink for automated
verification without depending on a real user terminal or network cache.

## Rendering

Use one transient stderr line with stable labels for all metrics, for example:

```text
checks 3/12 complete · built 8 · to build 4 · cached 16 · to fetch 2 · known total 30
```

The initial snapshot is the first change from no displayed metrics. Subsequently,
render only when the displayed metric tuple changes, including availability.
Coalesce queued derivation events before computing and rendering a snapshot;
individual work events do not scan the accumulated derivation state or write to
the terminal. Check finalization remains visible during active work.
Duplicate or irrelevant events, elapsed time, idle polling, and unchanged activity
messages produce no writes. Do not animate a spinner or repeat unchanged status
lines. Serialize concurrent updates so counts cannot regress through stale
snapshots; classification changes may legitimately change individual categories.

Erase or replace the previous status line without accumulating a line per event.
Use a compact representation on narrow terminals without wrapping; omit decorative
separators before abbreviating labels. Width comes from the progress stream's
terminal; `COLUMNS` is only a conservative fallback when the platform query is
unavailable. Color follows the shared output policy.
Before the final summary, error, or interruption report, clear the transient line
and restore the cursor to a clean line. Cleanup is not a new metric event and must
not repeat an unchanged snapshot. Live rendering never changes the JSON
contract, retained outcome ordering, or exit semantics; only the eligible
completion summary adds the final metrics line. Disable live
rendering if its sink fails; retain results and attempt normal final reporting.

## Final metrics

When live progress is eligible, the completion summary closes with a compact
final work status: `built`, `cached`, and `total`, width-clamped like the
transient line. Include `pending` when unfinished build or substitution work
remains after failure or cancellation, so the total has no unexplained gap.
The check-completion counter is omitted because the summary already reports
check outcomes. The snapshot is the last applied tuple, including
unknown availability; the status counts against the bounded summary budget.

When progress is ineligible, metrics were not collected and the completion
output omits them entirely. JSON completion documents never carry a metrics
field or prose line.
