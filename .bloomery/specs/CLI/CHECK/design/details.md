# Retained runs and seekable details

## Storage and identity

Store execution results in a workspace-scoped user cache outside the repository,
using the platform user-cache location (honoring `XDG_CACHE_HOME` where
applicable). Namespace by canonical workspace root so results from different
workspaces cannot be mixed. Use opaque run IDs, not timestamps as the sole
identity. Retain the selection, system scope, selected check outcomes and the
`format:workspace` outcome, structured failures, Git revision and working-tree
state when available, and captured logs.
Sample revision and working-tree state after the formatter gate completes or
fails, so retained metadata describes the source used by check tasks and any
formatting edits, including partial edits from a failed formatter.
A missing Git command or unavailable revision/status leaves the corresponding
field null and does not fail the run. Working-tree state includes normal
untracked files. When a selected check fails during batched realization, retain its derivation
store path with the failure record. On `check details`, resolve that path with
`nix log` once and retain its normalized output or unavailability result in a
shared run-local derivation snapshot. Keep the batched realization output;
failure to retrieve a referenced log does not hide retained details or change
the failed-check outcome.

Allocate a run before execution. Write an atomic completed manifest after
execution; completed runs are immutable. Concurrent runs have distinct storage.
The default latest run is the latest completed run by completion time, never an
in-progress run. Interrupted runs finalized with partial results are completed
records with interrupted status. Do not silently fall back to an older run when
an explicitly selected run is missing.

Retention is explicit: do not automatically remove logs in the first version.
The user may delete the workspace cache, after which retrieval reports that the
run is unavailable. `check details` may run `nix log` for a retained derivation
path, but never reruns a check or build. Documentation must state that logs can
contain sensitive build output. Use user-private cache permissions and do not
upload logs.

## Failures

Assign short run-local IDs (`f1`, `f2`, ...) after sorting by check ID, diagnostic
code, subject, source location, and a deterministic occurrence tie-breaker.
Blocked/canceled/not_run checks do not produce duplicate root-cause failures.
Static diagnostics are individual failures; Nix failures are normally failed
check attributes. A formatter failure uses `format:workspace` and retains its
captured command output as the task log. Alias failures can share a retained log.

`check failures [--run RUN] [--offset N] [--limit N]` enumerates failure records,
not successful checks. Offset is zero-based in the sorted failure list. Default
is offset 0, limit 20; every page also has an 8 KiB byte ceiling. Report totals,
actual range, and next offset. An offset at or beyond the end yields an empty
page with no continuation; negative values and zero limits are usage errors.

`check details FAILURE [--run RUN] [--offset N] [--limit N]` retrieves data without
rerunning checks or builds. The first derivation-log retrieval may invoke
`nix log`; subsequent retrievals reuse its snapshot. Unknown IDs are explicit retrieval
errors. The run defaults to the latest completed run for this workspace.

## Detail stream and seek

Normalize captured Nix output and any on-demand derivation log into a stable
UTF-8 detail stream with LF separators, replacement for invalid UTF-8, and
terminal/control escape sequences removed. Retain source logs separately if
needed; display only the safe normalized stream.
Split oversized logical lines into bounded display records so seeking can reach
every retained part. Line offsets refer to these display records and remain
stable for the completed run. Snapshot derivation logs atomically under a
cross-process lock so concurrent retrievals resolve each run-local store path
once. Freeze unavailable-log records too; later log availability must not alter
totals or offsets. Shared aliases reuse one snapshot without modifying the
completed manifest. Snapshot cache failures are retrieval errors, not fallback
streams. Do not duplicate an entire shared log per failure.

Details include check ID, outcome, diagnostic code/message, relevant
repository-relative location or failed derivation when available, and a bounded
excerpt. Default page is at most 40 display records and 8 KiB across the whole
serialized response. Explicit `--offset` is zero-based. Without it, choose a
failure-focused excerpt: diagnostic context for static failures and the retained
log tail for unstructured build failures. Return the actual starting offset.

Human and JSON output carry the same paging metadata: offset, next (nullable),
and total display records. The next offset identifies the first unreturned
record. A single record must fit the byte budget after serialization; split
records and truncate metadata with explicit continuation where necessary.
Offsets beyond the end yield an empty page, not a rerun. Context and metadata
must not consume the budget so completely that paging makes no progress.

Example:

```text
bloomery check details f2 --run r42
bloomery check details f2 --run r42 --offset 0 --limit 20
bloomery check details f2 --run r42 --offset 20 --limit 20 --json
```

Retrieval commands exit zero when retrieval succeeds even if the stored check
failed, and exit 2 for invalid requests or unavailable retained data.
