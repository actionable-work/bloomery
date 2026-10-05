---
id: GRAPH
name: Bloomery Crate Derivation Graph
tagline: Lower Cargo lock data into a pure, per-crate Nix derivation graph.
description: |
  The derivation graph feature defines how Bloomery reads workspace manifests
  and locks, resolves active features and dependency edges, selects crate
  sources, and constructs the crate, binary, and check derivations exposed
  through every integration.
---

# Crate Derivation Graph

Bloomery evaluates the graph purely in Nix: manifests, `Cargo.lock`, and
`bloomery.lock` are read during evaluation. Derivations are realized by Nix
after evaluation completes, and no Cargo process runs during evaluation or
build.

## Design documents

- [Lock data and graph resolution](design/resolution.md)
- [Derivation contracts](design/derivations.md)
- [Manifest policy checks](design/manifest-checks.md)

## Scope

This feature covers graph inputs, workspace discovery, feature and dependency
resolution, feature unification modes, crate sourcing, crate identity, binary
discovery, build scripts, propagation metadata, asset flow, and the workspace
check derivations. Fileset selection mechanics are owned by
[NIX/SOURCES](../../NIX/SOURCES/README.md).