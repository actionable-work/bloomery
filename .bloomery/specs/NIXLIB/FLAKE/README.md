---
id: FLAKE
name: Bloomery Flake Interface
tagline: Expose one config-driven mkFlake over the workspace option schema.
description: |
  The flake interface feature defines the single mkFlake constructor Bloomery
  exports, the required config.toml build tables it evaluates, and the
  packages, apps, checks, formatters, and development shells produced for each
  system.
---

# Flake Interface

Bloomery is consumed through one integration point: the `mkFlake` constructor.
It accepts a nixpkgs instance, a workspace root, an optional systems list, an
optional `overrides` set, an optional `extraFormatters` set, an optional
`extraOutputs` callback, and an optional main-flake `self`. Every other build
setting, including which sub-flakes to compose, is read from the required
`.bloomery/config.toml` and evaluated into the same output shape for each
selected system.

## Design documents

- [Entry points and system expansion](design/entry-points.md)
- [Build configuration](design/configuration.md)
- [Workspace options](design/workspace-options.md)
- [Per-system outputs](design/output-contract.md)
- [Sub-flake composition](design/composition.md)
- [Default formatter](design/formatters.md)

## Scope

This feature covers the exported constructor, configuration loading and value
encoding, option evaluation, system enumeration, flake attribute mapping,
sub-flake composition and output elevation, the default formatter and its
ordering graph, and output naming and gating. How the derivations behind those
outputs compile belongs to [Derivation graph](../GRAPH/README.md).
