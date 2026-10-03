---
id: CHECK
name: Traceability Check
tagline: Deterministic validation of structure, grammar, references, and automated coverage.
description: |
  The check feature combines the shared PARSER service with relational
  verification and the `bloomery check` command. It succeeds only when every
  declared invariant holds and emits rustc-style diagnostics when validation
  fails.
---

# `bloomery check`

Check is the enforcing command. It validates the repository snapshot without
running its tests or build system. It consumes the configuration, resolved
specification model, and evidence registry produced by the
[PARSER service](../../PARSER/README.md).

## Design documents

- [Validation pipeline](design/pipeline.md)
- [Diagnostics](design/diagnostics.md)
- [Nix workspace check](design/workspace-check.md)

## Responsibilities

- Orchestrate configuration and workspace loading, requirement validation,
  static evidence extraction, and relational checks.
- Reject orphan references, automated requirements without evidence, and
  evidence attached to manual requirements.
- Return exit code `0` only for an entirely valid snapshot and `1` for a
  validation failure.
- Preserve deterministic ordering and source locations in diagnostics.

## Non-responsibilities

Check does not schedule reviews, execute tests, build derivations, mutate
documents, or maintain lifecycle state. The Rust command stays read-only; Nix
workspace orchestration is an external integration documented separately.
