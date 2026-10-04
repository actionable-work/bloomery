# Nix orchestration

## Integration direction

The shared CLI preflight requires a root `flake.nix` before every command. See
[CLI command and workspace requirements](../../INTERFACE/design/commands.md#flake-presence-preflight).
After that gate, `bloomery check` owns workspace formatting, static verification,
and test/build orchestration:

```text
bloomery check
├── execution: catalog/selection setup → format:workspace (`nix fmt`)
│   └── after formatting succeeds: static checks + Nix derivations
└── list/failures/details: shared flake gate only; no formatting
```

`mkWorkspace`, `mkFlake`, and flake-module integration do not generate a
`bloomery:check` output, irrespective of `.bloomery/` presence. Other generated
checks retain their enable/include-package gates. Nix check generation does not
depend on the input Bloomery CLI package for specification validation and has no
builder for the recursive check.

`nix flake check` remains independently usable, but no longer certifies static
Bloomery validation. Documentation and CI should present `bloomery check` as
the full testing entry point. Do not invoke `nix flake check` inside the runner.
For externally supplied flakes, exclude an exact legacy `bloomery:check`
attribute with an explicit compact compatibility notice, preventing the known
recursive integration; custom derivations must not invoke the runner themselves.

## Formatter preflight

For each valid `bloomery check` execution, invoke `nix fmt` once from the
workspace root, using the flake's default formatter. Run it regardless of check
selectors or selected systems, after catalog/selection setup and before any
selected check task. It is an implicit `format:workspace` outcome, not a
`checks.<system>.*` attribute or a selectable catalog ID. Read-only check
subcommands do not invoke the formatter, but still require the shared flake gate.

Capture formatter output in a run-local log without forwarding it to normal
output. A non-zero exit is the `NixFormatFailed` check failure described in
[selection and execution](execution.md#formatting-gate); failure to start Nix or
retain the log is operational and leaves the formatter and selected checks
not_run. Leave source edits in place, including partial edits from a failed
formatter. Disable lockfile writes and updates for this Nix operation.

## Internal wrapper

Use Nix's CLI directly. Do not require or invoke `nix-fast-build`, and do not
add a small orchestration dependency solely for scheduling.

After catalog discovery and the formatter preflight:

1. Invoke one argument-safe `nix build --no-link` for each selected check
   installable; Nix evaluates that attribute as part of the build request.
2. Schedule bounded concurrent build requests and capture output separately
   for each selected check. Consume structured Nix activity for run-scoped
   [derivation metrics](progress.md#derivation-metrics), preserving retained
   diagnostic and builder logs without forwarding activity to terminal output.
3. Map outcomes to their check IDs. Nix store locking deduplicates realization
   when aliases resolve to one derivation.

A catalog-wide discovery failure is an operational error; an individual
malformed/non-derivation check or build failure is a check failure. Treat an
installable evaluation error and a builder failure as the same check-failure
category, retaining the complete invocation output instead of inferring a
subtype from human diagnostics. Continue other discoverable attributes in
default mode. Do not silently report an incomplete catalog as a full passing
run.

Use argument-safe subprocess invocation, not shell interpolation of attribute
names. Keep every Nix evaluation pure; never pass `--impure`. Disable lockfile
writes/updates and result-link creation during all Nix operations. Honor users'
Nix configuration for stores, substitution, builders, and build parallelism.
Report unavailable Nix when a check execution requires formatting or selected
Nix checks.

Nix owns the dependency graph and deduplicates shared derivations through its
store locks; Bloomery schedules top-level build requests only. Submit each
selected attribute once and retain an outcome for each alias. A shared derivation
may be referenced by several outcomes without duplicating its retained logs.

A top-level failed derivation is a check failure, not necessarily an individual
test-case failure. Preserve individual test names only where reliably available
from structured output. Do not build a heuristic universal test-output parser.
