---
id: CHECK
name: Traceability Check
tagline: Deterministic validation of structure, grammar, references, and automated coverage.
description: |
  The check feature combines workspace parsing, requirement validation, static
  evidence scanning, and relational set checks into the `bloomery check`
  command. It returns success only when every declared invariant holds and
  emits rustc-style diagnostics when it does not.
---

# `bloomery check`

Check is the enforcing command. It validates the repository snapshot without
running its tests or build system.

## Design documents

- [Validation pipeline](design/pipeline.md)
- [Diagnostics](design/diagnostics.md)

## Responsibilities

- Parse configuration and construct the workspace graph.
- Validate service and feature document contracts.
- Parse requirement files and resolve design links.
- Merge static references from all enabled scanners.
- Reject orphan references, automated requirements without evidence, and
  evidence attached to manual requirements.
- Return exit code `0` only for an entirely valid snapshot and `1` for a
  validation failure.

## Non-responsibilities

Check does not schedule reviews, execute tests, build derivations, mutate
documents, or maintain lifecycle state.
