---
id: IMPLEMENTATION
name: CLI Implementation Architecture
tagline: Modular Rust architecture for parsing, scanning, validation, and output.
description: |
  The implementation feature records a reference Rust decomposition for the
  CLI service. It keeps command dispatch, parsing, domain modeling, scanners,
  validation, diagnostics, and formatting separable so behavior can evolve
  without returning to a monolithic implementation.
---

# CLI Implementation Architecture

These documents describe the implementation architecture for the behavior defined
by the other features. They are intentionally subordinate to the service and
feature contracts; implementation details may change without changing the
workspace or traceability model.

## Design documents

- [Module architecture](design/architecture.md)
- [Domain model](design/domain-model.md)
- [Tooling and dependency roles](design/tooling.md)
- [Nix workspace check](design/workspace-check.md)

## Responsibilities

- Keep CLI dispatch small and command-specific behavior modular.
- Represent one resolved graph for check and review.
- Isolate source-language parsers behind scanner interfaces.
- Preserve source locations and structured diagnostics through formatting.

## Non-responsibilities

The Rust CLI does not own workspace-build orchestration. When a workspace has
a `.bloomery/` directory, the Nix library uses the Bloomery package output from
the input flake to run `bloomery check`; that wrapper is described in
[Nix workspace check](design/workspace-check.md) and does not change the CLI's
read-only behavior.
