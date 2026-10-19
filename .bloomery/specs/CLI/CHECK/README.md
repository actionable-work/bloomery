---
id: CHECK
name: Format-Gated Check Runner
tagline: Formatting first, then concurrent static and batched Nix checks with bounded failure output.
description: |
  Check is the workspace testing entry point. Every command requires a root
  `flake.nix`. Check execution runs `nix fmt` before static specification
  validation and Nix check derivations; a formatting failure always stops the
  run. After formatting succeeds, static checks and one batched Nix realization
  run concurrently, with terminal work metrics and seekable retained failures.
  Code health shares Rust parsing with evidence scanning, reports crate and
  workspace scores, and compares a reviewed tracked baseline.
  Clippy suppression policy checks inline Rust attributes and rejects disabled
  Clippy lints unless their exact lint names are configured as exceptions.
  Unconfigured check controls default to off; `bloomery config upgrade` sets
  missing supported check controls to `true` and preserves configured values.
  Check warns through tracing when a supported check key is missing.
  [Build](../BUILD/README.md) is the production materialization
  gate, and `bloomery build --check` composes this suite as its prerequisite.
  Static specification validation runs only when specification management and
  `checks.specifications.enable` are enabled, so check also runs without
  specification checks.
---

# `bloomery check`

A full successful run is the workspace readiness gate. A successful subset run
certifies only its selection. The shared CLI preflight requires a root
`flake.nix` for every command. For every valid check execution, the mandatory
`nix fmt` preflight runs, including partial selections, and must succeed before
any selected check starts. Static evidence remains necessary but is not a
substitute for executing the selected Nix checks. Static specification checks
run only when specification management and `checks.specifications.enable` are
enabled. If either is disabled, check runs the formatter gate, independently
enabled code health and Clippy suppression policy, and Nix checks without
specification passes.

## Design documents

- [Static validation pipeline](design/pipeline.md)
- [Selection and parallel execution](design/execution.md)
- [Terminal progress and work metrics](design/progress.md)
- [Failure summaries and diagnostics](design/diagnostics.md)
- [Retained runs and seekable details](design/details.md)
- [Nix orchestration and integration removal](design/workspace-check.md)
- [Hermetic Nix boundary tests](design/testing.md)
- [Code health scores and reviewed snapshots](design/code-health.md)
- [Inline Clippy suppression policy](design/clippy-suppressions.md)

## Responsibilities

- Preserve structure, grammar, design-link, scanner, and traceability validation
  from the [PARSER area](../../PARSER/README.md) when specification management
  and `checks.specifications.enable` are enabled.
- Run the formatter gate, enabled code health, enabled Clippy suppression
  policy, and Nix checks without a specification tree when specification
  management is disabled.
- Require the shared root-flake preflight for every command.
- Run `nix fmt` before selected checks and stop on formatting failure.
- Run independent static and Nix work concurrently with bounded scheduling,
  realizing the selected Nix derivations in one batched invocation.
- Include package checks only when `checks.includePackageChecks` enables them;
  the default suite omits them.
- Discover and realize checks elevated from configured sub-flakes as ordinary
  check attributes of the main flake.
- Complete selected checks unless fail-fast, interruption, or a formatting
  failure prevents execution.
- Display check completion and deduplicated Nix work metrics only on user
  terminals.
- Report bounded failure summaries in human and JSON formats.
- Retain immutable results and logs for paginated retrieval without rerunning.
- Leave formatter changes in the workspace and leave repository lockfiles
  unchanged.
- Share Rust ingestion between health and traceability without coupling outcomes.
- Check inline Clippy lint suppressions against configured exceptions.
- Warn through tracing when a supported check-control key is absent.
- Retain explainable crate scores and a SLOC-weighted workspace score.
- Apply configurable health limits and baseline regression gates.
- Export reviewed health snapshots only through `check health update`.
- Exercise runner behavior, the Nix subprocess adapter, and shared Nix catalog
  expressions through unconditional checks in the default workspace suite.

## Boundaries

Check executions run `nix fmt` before validation and Nix builds. The shared
CLI preflight requires a root `flake.nix` for every command. Check writes a
private workspace-scoped user cache. `nix fmt` may modify source files; those
edits are not rolled back if formatting fails. Retained logs can contain
sensitive build output and are not removed automatically; delete the workspace
cache to remove them. The explicit `check health update` command writes the
configured tracked snapshot; ordinary execution and retrieval leave it unchanged.
Health uses source facts without compiler, coverage, or artifact measurements.
Check may use the network through Nix but leaves lockfiles unchanged. It
does not produce deployable package artifacts; [build](../BUILD/README.md)
materializes production packages. It
does not repair specifications, maintain locks, schedule human reviews, or
implement a second Nix dependency scheduler.
Generated Nix checks must not invoke the top-level runner recursively.
