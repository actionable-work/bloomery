---
id: FLAKE
name: Bloomery Flake Interface
tagline: Export mkFlake, mkLib, and flake-parts integrations over one workspace option schema.
description: |
  The flake interface feature defines the public constructors Bloomery
  exports, the categorized workspace options they evaluate, and the packages,
  apps, checks, and development shells produced for each system.
---

# Flake Interface

Bloomery is consumed through four integration points: the zero-boilerplate
`mkFlake` constructor, the per-system `lib.${system}` builder, the `mkLib`
constructor for custom package sets, and the flake-parts module. Each
integration evaluates the same workspace option module and returns the same
per-system output shape.

## Design documents

- [Entry points and system expansion](design/entry-points.md)
- [Workspace options](design/workspace-options.md)
- [Per-system outputs](design/output-contract.md)

## Scope

This feature covers exported constructors, option evaluation, system
enumeration, flake attribute mapping, and output naming and gating. How the
derivations behind those outputs compile belongs to
[Derivation graph](../GRAPH/README.md).