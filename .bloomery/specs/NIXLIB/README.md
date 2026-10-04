---
id: NIXLIB
name: Bloomery Nix Library
tagline: Exported flake interfaces and the pure Nix crate derivation graph.
description: |
  The NIXLIB area owns the reusable library surface Bloomery exports to
  consumers: the zero-boilerplate flake constructor, the per-system workspace
  builder, the flake-parts module, the strongly-typed options they evaluate,
  and the per-system outputs they return. It also owns how a workspace is
  lowered into a crate derivation graph from Cargo.toml, Cargo.lock, and
  bloomery.lock.
---

# Nix Library Area

The [Flake interface](FLAKE/README.md) feature defines the exported entry
points, the categorized workspace options, and the outputs each integration
produces.

The [Derivation graph](GRAPH/README.md) feature defines how lock data, feature
resolution, and crate sources become crate, binary, and check derivations.

## Boundaries

Source selection and rebuild isolation for those derivations are owned by
[NIX/SOURCES](../NIX/SOURCES/README.md). CLI check orchestration is owned by
[CLI/CHECK](../CLI/CHECK/README.md). Repository path organization is owned by
[REPOSITORY/LAYOUT](../REPOSITORY/LAYOUT/README.md).