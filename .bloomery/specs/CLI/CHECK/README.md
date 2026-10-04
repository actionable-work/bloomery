---
id: CHECK
name: Parallel Check Runner
tagline: Parallel checks with terminal progress and bounded failure output.
description: |
  Check is the workspace testing entry point. It runs static specification
  validation and Nix check derivations concurrently, completes all selected
  checks by default, displays changing work metrics on user terminals, and retains
  failures for bounded, seekable retrieval.
---

# `bloomery check`

A full successful run is the workspace readiness gate. A successful subset run
certifies only its selection. Static evidence remains necessary but is not a
substitute for executing the selected Nix checks.

## Design documents

- [Static validation pipeline](design/pipeline.md)
- [Selection and parallel execution](design/execution.md)
- [Terminal progress and work metrics](design/progress.md)
- [Failure summaries and diagnostics](design/diagnostics.md)
- [Retained runs and seekable details](design/details.md)
- [Nix orchestration and integration removal](design/workspace-check.md)

## Responsibilities

- Preserve structure, grammar, design-link, scanner, and traceability validation
  from the [PARSER area](../../PARSER/README.md).
- Run independent static and Nix work concurrently with bounded scheduling.
- Complete all selected checks unless fail-fast or interruption is requested.
- Display check completion and deduplicated Nix work metrics only on user terminals.
- Report bounded failure summaries in human and JSON formats.
- Retain immutable results and logs for paginated retrieval without rerunning.
- Leave repository source and lockfiles unchanged.

## Boundaries

Check executes tests and builds through Nix and writes a private workspace-scoped
user cache. Retained logs can contain sensitive build output and are not removed
automatically; delete the workspace cache to remove them. Check may use the
network through Nix. It does not repair specifications, maintain locks, schedule
human reviews, or implement a second Nix dependency scheduler.
Generated Nix checks must not invoke the top-level runner recursively.
