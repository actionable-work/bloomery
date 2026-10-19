# Workspace write policy

Use the global boolean `--no-write` to prevent mutations to the workspace.
For `init`, the target directory is the workspace. The flag is accepted before
or after every application command and nested command. Parser help and version
output remain available.

The policy covers project files, including source files, configuration,
specifications, and lockfiles. It permits normal output streams, Bloomery's
private run cache, and Nix store outputs required to execute checks or realize
builds. A build therefore remains available with `--no-write`; its nested
`--check` gate inherits no-write check behavior.

## Operations that require workspace writes

Reject a write-required operation with a usage error (exit code 2) before
preflights or command work can create or modify a file. This includes
`init`, `sync`, `spec add`, `spec set`, `spec remove`, `config set`,
`config unset`, a writing `config upgrade`, `config document`, and
`check health update`. Reject these operations even when they would otherwise
be idempotent. In particular, a rejected config mutation must not create a
missing `.bloomery/config.toml`. The existing read-only `config upgrade
--dry-run` and `--diff` modes remain available with `--no-write`; their
existing mutual exclusion still applies.

## Operations allowed under no-write

Read operations accept `--no-write` and proceed normally. These include check
retrieval, `review`, `disk`, config reads, and spec reads and projections.
Build planning and realization are also allowed; realization writes only to the
Nix store.

## Check execution

Without `--no-write`, every valid check execution runs `nix fmt` before
selected checks. It leaves formatter edits in the workspace. When
`static:code-health` is enabled and selected, check compares its measurement
with the configured baseline and atomically replaces the baseline after a
complete measurement. Refresh occurs even when health policy gates or unrelated
checks fail. A failed formatter preflight, incomplete health measurement,
or cancellation or interruption before measurement completes does not refresh
the baseline. Health remains opt-in and selectors continue to control its
measurement.

With `--no-write`, check does not run `nix fmt`. It includes the native
`checks.<system>."core:treefmt-check"` derivation for every selected system,
even for partial selections. A missing attribute is an operational catalog
error. The derivation follows
regular Nix check selection, batching, failure retention, and fail-fast rules.
It checks the files in its configured source set; untracked files omitted by
that source set are not validated. All selected checks, including health, run
against the workspace as found. Health may read and compare the configured
baseline but never replaces it. The run cache and Nix store can still change.

Check run records and summaries identify whether the run used write mode or
no-write mode so retrieval describes the behavior that produced the result.
