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

The `mkFlake` integration does not generate a `bloomery:check` output,
irrespective of `.bloomery/` presence. Other generated checks retain their
enable/include-package gates. Nix check generation does not
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

## Catalog evaluation

`bloomery check` evaluates each selected system's check catalog once. One
`nix eval` per selected system returns every non-legacy `checks.<system>`
attribute together with its derivation path. This derivation plan is the sole
evaluation input to realization; discovery and derivation-path evaluation are
not repeated per attribute. Read-only catalog listing may evaluate attribute
names without derivation paths.

An attribute that cannot produce a derivation path is recorded as that check's
evaluation failure while the remaining attributes continue to evaluate. A
catalog-wide evaluation failure is an operational error. Do not silently report
an incomplete catalog as a full passing run. Evaluation remaining pure and
lockfile writes being disabled apply to this phase.

Catalog evaluation also resolves the selected derivations' output and
dependency metadata in a single structured query, so substitution and build
activity can be attributed to canonical derivation paths without a query per
check.

## Batched realization

Realize the selected derivation plan with one `nix build --no-link` invocation
for the run, passing each evaluated derivation as an output installable
(`<drv>^*`) so no flake attribute is re-evaluated. Nix owns the dependency graph
and deduplicates shared derivations through its store locks; Bloomery submits the
plan once and retains an outcome for each selected check.

Default execution passes `--keep-going`, so Nix attempts every derivation whose
prerequisites permit realization, including siblings of a failing derivation.
`--fail-fast` omits `--keep-going` and cancels the invocation when the first
failed derivation is observed, leaving unstarted selected checks not_run.

Resolve each selected check's outcome from per-derivation realization evidence
and one post-realization output-validity query over the plan's output paths, not
from the invocation's aggregate exit status. A check whose derivation outputs are
valid is passed; a check whose outputs are absent is failed, except where
fail-fast left it unstarted. Aliases share the derivation outcome while retaining
individual check records and log references; a shared derivation may be
referenced by several outcomes without duplicating its retained logs.

Retain the batch invocation output once. It contains one error block per failed
top-level derivation, each naming its derivation path and, when a builder ran,
its `nix log` command. Segment those blocks by derivation path to seed per-check
failure details; pass/fail never depends on the localized block text. A check
that failed only because a dependency failed retains that causal failed
derivation path from the plan's dependency metadata, so `check details` can
resolve a useful log.

Use Nix's CLI directly. Do not require or invoke `nix-fast-build`, and do not
add a small orchestration dependency solely for scheduling. Use argument-safe
subprocess invocation, not shell interpolation of attribute names or store
paths. Disable lockfile writes/updates, result-link creation, and impure
evaluation during all Nix operations. Report unavailable Nix when a check
execution requires formatting or selected Nix checks.

Honor the user's Nix configuration for stores, substitution, builders, and
parallelism. The batched realization is one Bloomery-owned task and does not
impose a global process bound or terminate unrelated Nix clients.

The structured realization stream and the post-realization validity result feed
run-scoped [derivation metrics](progress.md#derivation-metrics) and are not
forwarded to terminal output. Never create per-attribute build requests. A failed check
retains its derivation path, and `check details` resolves that path with
`nix log` on demand.

A failed derivation is a check failure, not necessarily an individual test-case
failure. Preserve individual test names only where reliably available from
structured output. Do not build a heuristic universal test-output parser.

## Aggregate realization

Aggregate realization is owned by the runner: one batched derivation-path build
replaces a generated symlink or aggregate check output. The catalog therefore
exposes only real checks, and external flakes without an aggregate output are
realized the same way.
