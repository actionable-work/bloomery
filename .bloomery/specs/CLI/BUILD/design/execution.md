# Production realization

## Catalog evaluation

`bloomery build` evaluates each selected system's production catalog once. One
`nix eval` per system returns every selected `packages.<system>` attribute
together with its production derivation path. This derivation plan is the sole
evaluation input to realization; packages and derivation paths are not
re-evaluated per attribute.

The evaluated derivation is the exported package target. For a binary enabled
in `[optimize]`, that is the optimized derivation; the release and dev builds
supply checks, apps, and metadata only. Library packages are evaluated when
library package visibility is enabled. Dev-profile derivations are never
evaluated or realized by build.

An attribute that cannot produce a derivation path is recorded as that
package's evaluation failure while the remaining attributes continue. A
catalog-wide evaluation failure is an operational error. Evaluation remains
pure and disables lockfile writes.

## Batched realization

Realize the selected derivation plan with one `nix build --no-link` invocation
for the run, passing each evaluated derivation as an output installable
(`<drv>^*`) so no flake attribute is re-evaluated. Nix owns the dependency graph
and deduplicates shared derivations through its store locks; Bloomery submits the
plan once and retains an outcome for each selected package.

Default execution passes `--keep-going`, so Nix attempts every derivation whose
prerequisites permit realization. A package whose derivation outputs are valid
is passed, including a derivation Nix substitutes from a cache; a package whose
outputs are absent is failed. Resolve each package's output store path from its
derivation's realized outputs and report it.

A failed derivation is a package failure. Retain its derivation path so a user
can retrieve the log with `nix log`; build does not retain a run store.

Use Nix's CLI directly with argument-safe subprocess invocation, not shell
interpolation of attribute names or store paths. Disable lockfile writes and
updates, result-link creation, and impure evaluation during all Nix operations.
Honor the user's Nix configuration for stores, substitution, builders, and
parallelism. Report unavailable Nix as an operational error.

## Check gate

Without `--check`, build goes straight to catalog evaluation and realization.
With `--check`, run the default `bloomery check` suite for the selected systems
before catalog evaluation and realization, using the same execution as a
no-selector check invocation: the
formatter preflight runs, static validation and the generated check set run,
and the run is retained. The gate is a prerequisite of every package; build
evaluates and realizes no package until it passes.

A passed gate lets realization proceed. A failed gate realizes no selected
package, returns exit code 1, and reports the failing check IDs; because the
gate retains its run, its failures remain retrievable through
`bloomery check failures` and `bloomery check details`. An operational failure
in the gate returns exit code 2 and realizes no package. User interruption
during the gate returns exit code 130 and treats every package as not_run.

## Interruption

User interruption cancels the batched realization and retains completed package
outcomes. Build distinguishes canceled started work from not_run work. Use
process ownership to avoid terminating unrelated Nix clients; shared daemon or
remote builds may continue.

## Output

Eligible user terminals show per-package completion and deduplicated Nix work
metrics as a transient line, never in CI or JSON mode. Build does not stream
raw Nix build logs. When `--check` is selected, the gate's check progress and
outcomes are shown on the same terms as `bloomery check`.

Plain output lists each selected package, its system, its status, and its output
store path, followed by a summary count of passed and failed packages. When
`--check` is selected, output first reports the gate status and, on failure, the
failing check IDs. A failed
package names its derivation path and the `nix log` command that retrieves its
build log.

`build --json` emits one object with `command: "build"`, `status`, the selected
`systems`, and a `packages` array. Each entry carries `name`, `system`,
`status`, and, when realized, `path`; a failed entry also carries its
derivation path. When `--check` is selected, the object also carries the gate
`status` and, on failure, the failing check IDs. Usage and operational failures
emit one structured error
document retaining their normal exit codes. See the
[output contract](../../INTERFACE/design/output.md).
