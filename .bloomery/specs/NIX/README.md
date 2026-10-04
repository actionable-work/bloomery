---
id: NIX
name: Nix Workspace Builds
tagline: Pure Nix Rust builds with explicit, narrowly scoped inputs.
description: |
  The NIX area owns the reusable Rust workspace builder's source and derivation
  dependency contracts. It covers generated packages and Rust checks exposed
  through the exported mkFlake constructor. CLI check execution and repository
  path organization belong to their respective areas.
---

# NIX Area

The [Sources](SOURCES/README.md) feature defines fileset selection and source
identity for Rust builds, checks, assets, and auxiliary inputs. The exported
`mkFlake` constructor that consumes these sources and the crate derivation
graph are specified by [NIXLIB](../NIXLIB/README.md).

This area does not own [CLI check orchestration](../CLI/CHECK/README.md),
[repository layout](../REPOSITORY/LAYOUT/README.md), or documentation rendering.
