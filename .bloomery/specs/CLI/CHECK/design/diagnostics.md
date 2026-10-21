# Failure summaries and diagnostics

## Static diagnostic vocabulary

Preserve the existing diagnostic semantics and repository-relative locations:

| Code | Meaning |
| --- | --- |
| `DirectoryIdMismatch` | Directory and README ID disagree. |
| `MissingDocument` | Required README is absent. |
| `MissingDesignFile` | Design path cannot be resolved. |
| `DanglingDesignAnchor` | Referenced Markdown heading is absent. |
| `SpecIdPathMismatch` | Requirement ID disagrees with its path. |
| `DuplicateSpecId` | Requirement ID is declared more than once. |
| `OrphanTestReference` | Evidence names no declared requirement. |
| `IllegalManualAutomation` | Evidence references a manual requirement. |
| `MissingAutomatedTest` | Automated requirement has no evidence. |
| `ParseError` | Configuration, Markdown, TOML, source, or Nix input is invalid. |
| `HealthScoreBelowMinimum` | A crate or workspace score is below its configured floor. |
| `HealthFunctionLimitExceeded` | A callable exceeds a configured hard metric limit. |
| `HealthRegression` | A comparable score decreased beyond the permitted allowance. |
| `HealthBaselineMissing` | Applicable health has no baseline file to compare. |
| `HealthBaselineInvalid` | The baseline is malformed or unsupported. |
| `HealthBaselineIncompatible` | Baseline and current scoring or selection policy differ. |
| `HealthBaselineWriteFailed` | The complete health measurement could not replace the configured baseline. |

Health source read/parse failures retain source diagnostics under
`static:code-health`; shared failures carry affected consumer IDs. Health policy
diagnostics include the measured value, configured limit or prior score, crate
identity, and contributing source spans where applicable. See
[code health](code-health.md#policy-gates).

Runner failures retain the batched realization output and the failed check's
derivation store path as seekable task details. A write-mode formatter failure
is recorded under `format:workspace` with its captured formatter output. A
no-write native treefmt failure is recorded under its ordinary Nix check ID. The
`check details` command resolves a retained Nix store path on demand and includes
the derivation log in its detail stream. Setup/discovery errors are operational
failures, not fabricated test cases.
Diagnostics do not cause automatic repairs. Preserve complete structured
messages, locations, and notes in the run store; default output is a summary,
not the complete compiler-style rendering.

## Bounded summary

Print one aggregate outcome line, a failure page, and any relevant retrieval
hint. The summary uses the shared
[presentation grammar](../../INTERFACE/design/presentation.md): the shared status
glyph and label, the semantic color roles for failure IDs and source locations,
and a closing summary. On eligible user terminals, close the summary with the
final derivation work metrics from
[terminal progress](progress.md#final-metrics). Do not print individual
successes, build logs, durations, repeated headings, zero failure-record counts,
or expanded diagnostic notes by default. Successful full runs report the total
as a check count after the passed status, without repeating `passed`. Write mode
is implicit; identify `no-write` only when that mode was selected. If a health
report was retained, show only its run-specific retrieval command. Do not
announce an automatic baseline refresh or print the baseline path in the summary.
The command supplies the run ID, so do not repeat it in a separate run footer or
notice.
Failure-detail and continuation commands likewise carry the run ID without a
separate run footer. When no retrieval command is present, show the run ID once.
Subset runs identify partial scope. Eligible user terminals display one transient stderr
line for changing check and derivation metrics; CI, non-TTY, and JSON output have
no progress stream. See [terminal progress](progress.md). Transient updates are
separate from the bounded final summary and do not retain a line per task.

Default failure page is at most 20 records and 8 KiB for the whole serialized
response. Bound UTF-8 bytes and records, not model-dependent token counts. The
budget includes headings, JSON syntax/escaping, counts, metadata, and retrieval
hints. Bound individual summary fields so at least one failure fits. Truncation
must be explicit; the full retained diagnostic remains available through details.

```text
✗ failed · 28 passed · 2 failed · 1 blocked · 2 failure records
f1 static:traceability MissingAutomatedTest CLI-CHECK-RUN-001
f2 nix:x86_64-linux:core:unit-tests BuildFailed
details: bloomery check details f2 --run r42
built 12 · cached 4 · total 18
```

A successful full run with a completed health measurement is rendered as:

```text
✓ passed · 267 checks
health report: bloomery check health --run r29118-18e09712a4059892-0
built 0 · cached 142 · total 142
```

Report selected-check outcome counts and the write-mode `format:workspace`
outcome separately from the total number of failure records: one static check
may produce many diagnostics. Show the failure-record total only when it is
nonzero. Omitted failures have a count and continuation offset; `check failures`
retrieves subsequent pages. Retain the run mode in the run record and identify
no-write mode in the summary. Notices (such as legacy integration exclusions)
are bounded too and cannot silently consume the failure page. Details are
described in [retained runs](details.md).

## JSON

One compact document on stdout under the shared
[JSON envelope](../../INTERFACE/design/output.md#json-envelope); the fields below
are the envelope's `result` payload. Readable keys, absent optional fields
omitted, no duplicated human prose, no successful-check array, no tool output.
Outcome
counts include passed, failed, blocked, canceled, and not_run, omitting zeros.
Run status is passed, failed, error, or interrupted. Run identity is absent only
when failure occurs before creating a run. A scope object is present for partial
selection. Failure records have id, check, code, and optional subject/location.
Paging includes total failure records, offset, and nullable next.

```json
{"command":"check","operation":"run","status":"failed","result":{"run":"r42","status":"failed","counts":{"passed":28,"failed":2,"blocked":1},"total":2,"offset":0,"failures":[{"id":"f1","check":"static:traceability","code":"MissingAutomatedTest","subject":"CLI-CHECK-RUN-001"},{"id":"f2","check":"nix:x86_64-linux:core:unit-tests","code":"BuildFailed"}],"next":null}}
```

Usage/setup errors use a bounded error object with code and message and retain
the appropriate exit code. JSON retrieval pages use the same failure record
shape, or a details object with check identity, bounded metadata, display
records, and offset/next/total. Color is terminal-only and honors `NO_COLOR` and
`TERM=dumb`. Human and JSON modes convey equivalent outcomes and paging data.

For identical retained results, serialization and ordering are stable. Run IDs,
completion times, logs, and fail-fast outcomes can differ between executions;
do not claim byte-identical JSON across separate runs.
