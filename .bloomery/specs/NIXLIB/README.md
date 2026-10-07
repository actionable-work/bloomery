---
id: NIXLIB
name: Bloomery Nix Library
tagline: The exported mkFlake interface and the pure Nix crate derivation graph.
description: |
  The NIXLIB area owns the reusable library surface Bloomery exports to
  consumers: the config-driven mkFlake constructor, the workspace option
  schema it evaluates from `.bloomery/config.toml`, and the per-system outputs
  it returns. It also owns how a workspace is lowered into a crate derivation
  graph from Cargo.toml, Cargo.lock, and bloomery.lock.
---

# Nix Library Area

The [Flake interface](FLAKE/README.md) feature defines the exported `mkFlake`
entry point, the required configuration table it reads, and the outputs it
produces.

The [Derivation graph](GRAPH/README.md) feature defines how lock data, feature
resolution, and crate sources become crate, binary, and check derivations.

The [Optimized binary generation](OPTIMIZE/README.md) feature defines how a
binary is optionally built with profile-guided and BOLT optimization driven by
a user-provided training script.

## Boundaries

Source selection and rebuild isolation for those derivations are owned by
[NIX/SOURCES](../NIX/SOURCES/README.md). CLI check orchestration is owned by
[CLI/CHECK](../CLI/CHECK/README.md). Repository path organization is owned by
[REPOSITORY/LAYOUT](../REPOSITORY/LAYOUT/README.md).