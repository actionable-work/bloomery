# Selection and parallel execution

## Check catalog

| ID | Responsibility | Prerequisite |
| --- | --- | --- |
| `static:structure` | Configuration, workspace/frontmatter, requirements, design links | specification management and enabled specification checks |
| `static:traceability` | Enabled scanners and relational evidence checks | enabled specification checks and loaded valid requirement model |
| `static:code-health` | Rust analysis, health scores, limits, baseline comparison | enabled health policy |
| `static:clippy-suppressions` | Inline Clippy suppression policy | enabled suppression policy |
| `nix:<system>:<attribute>` | One attribute under `checks.<system>` | Catalog evaluation |

Preserve the existing static validations; these IDs group the existing passes,
not new independent validation semantics. Static structure is a prerequisite
of traceability. `static:structure` and `static:traceability` exist in the
catalog only when specification management is enabled and
`checks.specifications.enable` is true; otherwise they are absent and cannot
be selected. The shared CLI preflight requires
a root `flake.nix` before any
check operation. Nix catalog discovery is setup and does not depend on
specification validity. The formatter gate runs before selected check tasks; a
failure prevents static validation and Nix check realization. After it passes,
failures in either pipeline do not suppress the other in default mode.

`static:code-health` exists when `checks.codeHealth.enable` is true, independently
of specification management and Nix check generation. It has no structure or
traceability prerequisite. Health and traceability share Rust ingestion without
coupling outcomes; see [code health](code-health.md#check-integration).
`static:clippy-suppressions` follows its independent enablement and does not
require specification management or Nix check generation; see
[Clippy suppressions](clippy-suppressions.md#check-integration).

## Check enablement warnings

Each configurable check category has a boolean check-control key in the raw
`.bloomery/config.toml` checks table. Omitted keys have an effective value of
`false`; check loading retains their raw absence so `bloomery check` can emit a
warning-level tracing event for each supported check whose check-control key is
missing. Warnings identify the check and missing key, and recommend
`bloomery config upgrade`, which materializes the key as `true`. An explicitly
configured `false` is intentional: it
produces no warning and remains disabled. Static structure and traceability
checks require both `specs.enable` and `checks.specifications.enable`.
Specification-check warnings are absent when specification management is
disabled and those checks are unavailable.
An explicit `checks.enable = false` suppresses warnings for its Nix check
controls. Explicitly disabling a check group suppresses warnings for checks in
that group. Independently configured CLI checks such as code health and Clippy
suppression policy retain their own warning behavior.

With no selectors, run every available check: static specification checks when
specification management and `checks.specifications.enable` are enabled,
enabled code health and Clippy suppression policy, plus every check attribute
for the host Nix system. Include generated
Rust tests, clippy, documentation, doctests, lock validation, formatter checks,
and integration-flake checks when present in that output. Package checks are
included only when `checks.includePackageChecks` enables them; an omitted key
leaves them out and
[build](../../BUILD/README.md) materializes production packages instead. Do not
implicitly build packages outside checks
or select other architectures. Every check command requires the shared flake
preflight, including static-only and read-only selections. When specification
management is disabled, a selector naming a static check matches no catalog
entry and is rejected like any other unmatched selector. Code health selectors
remain valid while specification management is disabled.

## Formatting gate

For each valid `bloomery check` execution, run `nix fmt` once from the workspace
root after selection validation and run-store allocation, but before admitting
any selected check. The gate runs for full and partial selections, including
selections containing only static checks. It is not a catalog entry and cannot
be selected; `check list`, `check failures`, and `check details` require the
shared flake preflight but do not run the formatter.

Record the gate as the implicit `format:workspace` outcome. Capture its output
in the run store without streaming it to normal output. A successful formatter
outcome is passed, and selected checks run against the resulting workspace.
Keep formatter edits in the working tree, including edits made before a
formatting failure; do not roll them back.

A non-zero `nix fmt` exit is a formatter check failure, not an operational
failure. Record `format:workspace` as failed with a `NixFormatFailed` failure
record, mark every selected check `not_run`, and finalize the run as failed with
exit code 1. Publish those final outcomes to progress. This is unconditional
fail-fast: no selected check starts, regardless of `--fail-fast`. Failure to
start Nix or retain the formatter log is an operational error with exit code 2;
leave the formatter and selected checks not_run and retain the partial run.
User interruption retains the usual interrupted status and exit code.

## Selection

`--check ID_OR_GLOB` is repeatable; selections form a deduplicated union. Exact
IDs and case-sensitive anchored globs are accepted. Glob syntax is `*` (zero
or more characters) and `?` (one character), including colons; no bracket or
escape syntax. Quote globs to avoid shell expansion. Every selector must match
at least one catalog entry or the invocation fails before executing checks.

Selecting traceability includes its structure prerequisite. Selecting only Nix
must not require loading specifications. `check list` discovers IDs without
executing checks and applies the same selectors. Sort IDs lexicographically and
page with zero-based `--offset` (default 0), positive `--limit` (default 20), and
an 8 KiB serialized response ceiling. Return actual offset, total, and nullable
next offset in both output formats. Report discovery errors rather than
pretending the catalog is complete. List is not a persisted execution run.
Catalog pages preserve complete check IDs. An ID over 4 KiB, or one that cannot
fit alone with metadata within the response ceiling, produces an explicit
operational error at its offset. Never return a non-advancing continuation.

`--system SYSTEM` is repeatable and selects the systems whose Nix catalogs are
included; omitted means the host Nix system. Deduplicate explicit systems. An
explicitly requested missing system is an error, never an implicit skip. Systems
with valid empty check sets are distinct from missing system outputs.

## Scheduler

After the formatter gate passes, run independent specification static checks,
health, Clippy suppression policy, and Nix tasks concurrently. Admit
traceability as soon as structure passes and a task slot is available, ahead of
queued independent work; do not wait for Nix completion.
The Nix branch is one batched realization task; Nix realizes independent
derivations concurrently within it. All tasks share admission and cancellation.
`--jobs N` is positive, defaults to the available logical CPU count, and has a
minimum of one. The limit applies to Bloomery-owned active tasks, not the serial
formatter preflight, catalog evaluation, the batched realization, or all
builders and dependencies inside the Nix daemon. Do not override users' Nix
builder configuration to imply a global process bound.

Track each selected check and `format:workspace` as passed, failed, blocked,
canceled, or not_run. Queued/running are intermediate states.
Publish final-outcome transitions during execution to the
[progress aggregator](progress.md#check-metrics); provisional not_run records do
not mark queued work complete. Default execution attempts every
selected check whose prerequisites permit execution; independent work continues
after validation, evaluation, or build failures. A failed static prerequisite
leaves its dependents blocked with a link to the causal failure, not extra
duplicate failure records. A failed Nix dependency marks each selected check
that resolves to it failed. Cached Nix realizations count as passed.

With `--fail-fast`, the first observed failure stops new admission and requests
cancellation of Bloomery-owned running tasks, including the batched realization.
Record queued work as not_run and unfinished started work as canceled. Already
completed outcomes are preserved. Use process ownership to avoid terminating
unrelated Nix clients. Shared daemon or remote builds may continue; do not
promise instantaneous cancellation. The first observed failure can differ with
timing; final presentation order must not depend on completion order.

## Exit and setup failures

| Code | Meaning |
| --- | --- |
| `0` | The formatter gate and every selected check passed |
| `1` | The formatter gate or a selected check failed, or failure prevented completion |
| `2` | Invalid invocation, unavailable required tool, discovery/setup or cache failure |
| `130` | User interruption |

Zero selected checks is a usage error. User interruption requests cancellation
and retains completed results. Capture realization output in run-local logs
rather than streaming it to normal output; resolve per-check failure logs from
their derivation paths. Create the run store before execution; inability to
create it is a setup error. Failure to finalize the store cannot produce exit
code zero. Tool/discovery failures are distinct from an individual check's
evaluation or build failure. Retain partial results when an operational failure
occurs after execution begins. See [Nix orchestration](workspace-check.md) and
[details storage](details.md).

The default full run is the readiness gate. Label any filtered or explicitly
system-selected run with its selection; successful subsets must not claim that
the default workspace suite passed. A run succeeds only when its formatter gate
and every selected check pass.
