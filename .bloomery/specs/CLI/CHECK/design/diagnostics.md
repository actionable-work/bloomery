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

Runner failures retain captured Nix check invocation output and the referenced
store path as seekable task details. `check details` resolves a retained Nix
store path on demand and includes the derivation log in its detail stream.
Setup/discovery errors are operational failures, not fabricated test cases.
Diagnostics do not cause automatic repairs. Preserve complete structured
messages, locations, and notes in the run store; default output is a summary,
not the complete compiler-style rendering.

## Bounded summary

Print one aggregate outcome line, a failure page, and one retrieval hint. On
eligible user terminals, close the summary with the final derivation work metrics
from [terminal progress](progress.md#final-metrics). Do not
print individual successes, build logs, durations, repeated headings, or expanded
diagnostic notes by default. Successful full runs need only counts and run ID.
Subset runs identify partial scope. Eligible user terminals display one transient
stderr line for changing check and derivation metrics; CI, non-TTY, and JSON output
have no progress stream. See [terminal progress](progress.md). Transient updates
are separate from the bounded final summary and do not retain a line per task.

Default failure page is at most 20 records and 8 KiB for the whole serialized
response. Bound UTF-8 bytes and records, not model-dependent token counts. The
budget includes headings, JSON syntax/escaping, counts, metadata, and retrieval
hints. Bound individual summary fields so at least one failure fits. Truncation
must be explicit; the full retained diagnostic remains available through details.

```text
FAIL  28 passed · 2 failed · 1 blocked
f1 static:traceability MissingAutomatedTest CLI-CHECK-RUN-001
f2 nix:x86_64-linux:core:unit-tests BuildFailed
run r42 · details: bloomery check details f2 --run r42
built 12 · cached 4 · total 18
```

Report check outcome counts separately from the total number of failure records:
one static check may produce many diagnostics. Omitted failures have a count
and continuation offset; `check failures` retrieves subsequent pages. Notices
(such as legacy integration exclusions) are bounded too and cannot silently
consume the failure page. Details are described in [retained runs](details.md).

## JSON

One compact document on stdout; readable keys, absent optional fields omitted,
no duplicated human prose, no successful-check array, no tool output. Outcome
counts include passed, failed, blocked, canceled, and not_run, omitting zeros.
Run status is passed, failed, error, or interrupted. Run identity is absent only
when failure occurs before creating a run. A scope object is present for partial
selection. Failure records have id, check, code, and optional subject/location.
Paging includes total failure records, offset, and nullable next.

```json
{"run":"r42","status":"failed","counts":{"passed":28,"failed":2,"blocked":1},"total":2,"offset":0,"failures":[{"id":"f1","check":"static:traceability","code":"MissingAutomatedTest","subject":"CLI-CHECK-RUN-001"},{"id":"f2","check":"nix:x86_64-linux:core:unit-tests","code":"BuildFailed"}],"next":null}
```

Usage/setup errors use a bounded error object with code and message and retain
the appropriate exit code. JSON retrieval pages use the same failure record
shape, or a details object with check identity, bounded metadata, display
records, and offset/next/total. Color is terminal-only and honors `NO_COLOR` and
`TERM=dumb`. Human and JSON modes convey equivalent outcomes and paging data.

For identical retained results, serialization and ordering are stable. Run IDs,
completion times, logs, and fail-fast outcomes can differ between executions;
do not claim byte-identical JSON across separate runs.
