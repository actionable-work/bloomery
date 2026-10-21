---
id: CHECK
name: Format-Gated Check Runner
tagline: Formatting first, then concurrent static and batched Nix checks with bounded failure output.
description: |
  Check is the workspace testing entry point. Every command requires a root
  `flake.nix`. Write-mode check execution runs `nix fmt` before static
  specification validation and Nix check derivations. In no-write mode,
  the native treefmt check runs as a Nix derivation. Static checks and one
  batched Nix realization run concurrently, with terminal work metrics and
  seekable retained failures.
  Code health shares Rust parsing with evidence scanning, reports crate and
  workspace scores, compares the prior baseline, and refreshes it after a
  complete selected health measurement in write mode.
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
`flake.nix` for every command. Every valid write-mode check runs `nix fmt`
before selected checks start. With `--no-write`, check does not run `nix fmt`;
it includes the native `core:treefmt-check` Nix derivation for every selected
system and reports a catalog error if it is missing. That derivation checks
its configured source set without modifying workspace files. Static evidence
remains necessary but is not a substitute for executing the selected Nix checks.
Static specification checks
run only when specification management and `checks.specifications.enable` are
enabled. If either is disabled, check runs the native formatter check,
independently enabled code health and Clippy suppression policy, and Nix checks
without specification passes.

## Design documents

- [Static validation pipeline](design/pipeline.md)
- [Selection and parallel execution](design/execution.md)
- [Terminal progress and work metrics](design/progress.md)
- [Failure summaries and diagnostics](design/diagnostics.md)
- [Retained runs and seekable details](design/details.md)
- [Nix orchestration and integration removal](design/workspace-check.md)
- [Hermetic Nix boundary tests](design/testing.md)
- [Code health scores and baseline snapshots](design/code-health.md)
- [Inline Clippy suppression policy](design/clippy-suppressions.md)

## Responsibilities

- Preserve structure, grammar, design-link, scanner, and traceability validation
  from the [PARSER area](../../PARSER/README.md) when specification management
  and `checks.specifications.enable` are enabled.
- Run the native no-write formatter check, enabled code health,
  enabled Clippy suppression policy, and Nix checks without a specification
  tree when specification management is disabled.
- Require the shared root-flake preflight for every command.
- Format before selected checks in write mode; include the native treefmt
  derivation in no-write Nix check work for every selected system.
- Run independent static and Nix work concurrently with bounded scheduling,
  realizing the selected Nix derivations in one batched invocation.
- Include package checks only when `checks.packages.enable` enables them;
  the default suite omits them.
- Discover and realize checks elevated from configured sub-flakes as ordinary
  check attributes of the main flake.
- Complete selected checks unless fail-fast, interruption, or a write-mode
  formatter failure prevents execution.
- Display check completion and deduplicated Nix work metrics only on user
  terminals.
- Report concise, bounded failure summaries and retrieval pages in human and
  JSON formats.
- Retain immutable results and logs for paginated retrieval without rerunning.
- Leave write-mode formatter changes in the workspace and leave repository
  lockfiles unchanged.
- Share Rust ingestion between health and traceability without coupling outcomes.
- Check inline Clippy lint suppressions against configured exceptions.
- Warn through tracing when a supported check-control key is absent.
- Retain explainable crate scores and a SLOC-weighted workspace score.
- Apply configurable health limits and baseline regression gates.
- Refresh the health baseline after each complete selected health measurement
  in write mode, including after check failures; preserve it in no-write mode
  and retrieval commands.
- Exercise runner behavior, the Nix subprocess adapter, and shared Nix catalog
  expressions through unconditional checks in the default workspace suite.

## Boundaries

Write-mode check executions run `nix fmt` before validation and Nix builds.
The shared CLI preflight requires a root `flake.nix` for every command. Check
writes a private workspace-scoped user cache. Write-mode `nix fmt` may modify
source files; those edits are not rolled back if formatting fails. Retained logs
can contain sensitive build output and are not removed automatically; delete
the workspace cache to remove them. A write-mode health check refreshes its
configured baseline after a complete measurement; `--no-write` check execution
and retrieval leave it unchanged. No-write still permits private run-cache and
Nix store writes.
Health uses source facts without compiler, coverage, or artifact measurements.
Check may use the network through Nix but leaves lockfiles unchanged. It
does not produce deployable package artifacts; [build](../BUILD/README.md)
materializes production packages. It
does not repair specifications, maintain locks, schedule human reviews, or
implement a second Nix dependency scheduler.
Generated Nix checks must not invoke the top-level runner recursively.
