# Selection and parallel execution

## Check catalog

| ID | Responsibility | Prerequisite |
| --- | --- | --- |
| `static:structure` | Configuration, workspace/frontmatter, requirements, design links | specification management and enabled specification checks |
| `static:traceability` | Enabled scanners and relational evidence checks | enabled specification checks and loaded valid requirement model |
| `static:code-health` | Rust analysis, health scores, limits, baseline comparison and write-mode refresh | enabled health policy |
| `static:clippy-suppressions` | Inline Clippy suppression policy | enabled suppression policy |
| `nix:<system>:<attribute>` | One attribute under `checks.<system>` | Catalog evaluation |

Preserve the existing static validations; these IDs group the existing passes,
not new independent validation semantics. In write mode, `format:workspace` is
a mandatory preflight task, not a catalog entry. In no-write mode, the native
`nix:<system>:core:treefmt-check` catalog entry supplies formatting validation
for each selected system. Static structure is a prerequisite
of traceability. `static:structure` and `static:traceability` exist in the
catalog only when specification management is enabled and
`checks.specifications.enable` is true; otherwise they are absent and cannot
be selected. The shared CLI preflight requires
a root `flake.nix` before any
check operation. Nix catalog discovery is setup and does not depend on
specification validity. In write mode, the formatter gate runs before selected
check tasks; a failure prevents static validation and Nix check realization. In
no-write mode, the native formatter derivation is ordinary Nix check work in
the batched realization, and its failure does not suppress independent checks
in default mode.

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
included only when `checks.packages.enable` enables them; an omitted key
leaves them out and
[build](../../BUILD/README.md) materializes production packages instead. Do not
implicitly build packages outside checks
or select other architectures. Every check command requires the shared flake
preflight, including static-only and read-only selections. When specification
management is disabled, a selector naming a static check matches no catalog
entry and is rejected like any other unmatched selector. Code health selectors
remain valid while specification management is disabled.

## Formatting

For each valid write-mode `bloomery check` execution, run `nix fmt` once from
the workspace root after selection validation and run-store allocation, but
before admitting any selected check. The formatter runs for full and partial
selections, including selections containing only static checks. In no-write
mode, include `nix:<system>:core:treefmt-check` for every selected system,
even when a partial selector does not match it. A missing attribute is a
catalog setup error. Realize the check in the normal batched Nix invocation; do not invoke
`nix fmt` or `nix fmt -- --check`. This native derivation checks the files in
its own source set, so untracked files excluded by that source set are not
validated. `check list`, `check failures`, `check details`, and `check health`
require the shared flake preflight but do not run formatting.

Record the write-mode formatter as `format:workspace`. In no-write mode, retain
the native Nix check under its ordinary `nix:<system>:core:treefmt-check` ID
and capture it with the batched realization output. Write-mode selected check
derivation paths are evaluated after `nix fmt` so they use the resulting
workspace; no-write checks use the workspace as found. Keep write-mode
formatter edits in the working tree, including edits made before a formatting
failure; do not roll them back.

A non-zero write-mode `nix fmt` exit is a formatter check failure, not an
operational failure. Record `format:workspace` as failed with a
`NixFormatFailed` failure record, mark every selected check `not_run`, and
finalize the run as failed with exit code 1. This preflight failure prevents
check work regardless of `--fail-fast`. A no-write native formatter failure
is an ordinary Nix check failure under its native catalog ID; let independent
checks continue unless `--fail-fast` cancels them. Failure to start Nix or
retain formatter output is an operational error with exit code 2; retain the
partial run. User interruption retains the usual interrupted status and exit
code.

## Selection

`--check ID_OR_GLOB` is repeatable; selections form a deduplicated union. Exact
IDs and case-sensitive anchored globs are accepted. The write-mode
`format:workspace` preflight is not a selectable catalog entry. The native
formatter check is selected by its `nix:<system>:core:treefmt-check` ID. Glob
syntax is `*` (zero or more characters) and
`?` (one character), including colons; no bracket or escape syntax. Quote
globs to avoid shell expansion. Every selector must match at least one catalog
entry or the invocation fails before executing checks. In no-write execution,
the native formatter check is added for each selected system even when an
explicit selector does not match it; a direct selector match deduplicates with
this implicit inclusion. `check list` shows
native catalog entries and applies selectors without execution-only implicit
inclusions.

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

After the write-mode formatter gate passes, run independent specification
static checks, health, Clippy suppression policy, and Nix tasks concurrently.
In no-write mode, the native formatter check shares the batched Nix realization
with selected Nix checks while static checks run concurrently. Admit
traceability as soon as structure passes and a task slot is available, ahead of
queued independent work; do not wait for Nix completion.
The Nix branch is one batched realization task; Nix realizes independent
derivations concurrently within it. All tasks share admission and cancellation.
`--jobs N` is positive, defaults to the available logical CPU count, and has a
minimum of one. The limit applies to Bloomery-owned active tasks, but not the
write-mode serial formatter preflight, catalog evaluation, the batched
realization, or all
builders and dependencies inside the Nix daemon. Do not override users' Nix
builder configuration to imply a global process bound.

Track each selected check and the write-mode `format:workspace` task as passed,
failed, blocked, canceled, or not_run. Queued/running are intermediate states.
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
| `0` | Formatting validation and every selected check passed |
| `1` | Formatting validation or a selected check failed, or failure prevented completion |
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
system-selected run with its selection and write mode; successful subsets must
not claim that the default workspace suite passed. A write-mode run succeeds
only when its formatter preflight and every selected check pass. A no-write run
succeeds only when every selected check, including any implicitly selected
native formatter check, passes.
