# Selection and parallel execution

## Check catalog

| ID | Responsibility | Prerequisite |
| --- | --- | --- |
| `static:structure` | Configuration, workspace/frontmatter, requirements, design links | none |
| `static:traceability` | Enabled scanners and relational evidence checks | loaded valid requirement model |
| `nix:<system>:<attribute>` | One attribute under `checks.<system>` | Nix discovery and evaluation |

Preserve the existing static validations; these IDs group the existing passes,
not new independent validation semantics. Static structure is a prerequisite
of traceability. Nix discovery and execution do not depend on specification
validity. Failures in either pipeline do not suppress the other in default mode.

With no selectors, run both static checks and every check attribute for the
host Nix system. Include generated Rust tests, clippy, documentation, doctests,
package builds, lock validation, formatter checks, and integration-flake checks
when present in that output. Do not implicitly build packages outside checks
or select other architectures. A workspace without `flake.nix` defaults to
static checks only; explicit Nix selection in that workspace is an error.

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

Run independent static and Nix tasks concurrently. Admit traceability as soon
as structure passes and a task slot is available, ahead of queued independent
work; do not wait for Nix completion. All tasks share admission and cancellation.
`--jobs N` is a positive
integer, defaulting to the available logical CPU count, with a minimum of one.
The limit applies to Bloomery-owned active check tasks, not discovery processes
or all builders and dependencies inside the Nix daemon. Do not override users'
Nix builder configuration to imply a global process bound.

Track each selected check as passed, failed, blocked, canceled, or not_run.
Queued/running are intermediate states. Default execution attempts every
selected check whose prerequisites permit execution; independent work continues
after validation, evaluation, or build failures. A failed prerequisite leaves
its dependents blocked with a link to the causal failure, not extra duplicate
failure records. Cached Nix realizations count as passed.

With `--fail-fast`, the first observed failure stops new admission and requests
cancellation of Bloomery-owned running tasks. Record queued work as not_run and
unfinished started work as canceled. Already completed outcomes are preserved.
Use process ownership to avoid terminating unrelated Nix clients. Shared daemon
or remote builds may continue; do not promise instantaneous cancellation.
The first observed failure can differ with timing; final presentation order
must not depend on completion order.

## Exit and setup failures

| Code | Meaning |
| --- | --- |
| `0` | Every selected check passed |
| `1` | Selected check failed, or failure prevented completion |
| `2` | Invalid invocation, unavailable required tool, discovery/setup or cache failure |
| `130` | User interruption |

Zero selected checks is a usage error. User interruption requests cancellation
and retains completed results. Capture per-task logs rather than streaming them
to normal output. Create the run store before execution; inability to create it
is a setup error. Failure to finalize the store cannot produce exit code zero.
Tool/discovery failures are distinct from an individual check's evaluation or
build failure. Retain partial results when an operational failure occurs after
execution begins. See [Nix orchestration](workspace-check.md) and
[details storage](details.md).

The default full run is the readiness gate. Label any filtered or explicitly
system-selected run with its selection; successful subsets must not claim that
the default workspace suite passed.
