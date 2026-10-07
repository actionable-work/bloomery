---
id: OPTIMIZE
name: Bloomery Optimized Binary Generation
tagline: Optionally build PGO and BOLT optimized binaries from a user training script.
description: |
  The optimization feature defines how a workspace opts a discovered binary
  into profile-guided optimization and BOLT layout optimization, how a
  user-supplied training script is run to collect profiles, and how the
  resulting optimized package is produced and exposed.
---

# Optimized Binary Generation

A workspace can opt a discovered binary into an optimized build. The optimized
build reuses the release binary's resolved crate graph, selected sources,
toolchain, and active profile, and adds a training run that exercises the
workload described by a user-provided script.

## Design documents

- [Optimization pipeline](design/optimization.md)

## Scope

This feature covers opt-in configuration, the PGO and BOLT pipelines, the
training script and its environment, optimized package and app replacement,
validation, and rebuild isolation. The crate and binary derivations the pipeline builds on are
owned by [Derivation graph](../GRAPH/README.md). Cataloguing the configuration
table is owned by [PARSER/CONFIGURATION](../../PARSER/CONFIGURATION/README.md).
Source selection and invalidation boundaries for training inputs are owned by
[NIX/SOURCES](../../NIX/SOURCES/README.md).
