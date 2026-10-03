---
id: LAYOUT
name: Bloomery Repository Layout
tagline: Map the repository's source directories, Rust packages, Nix code, and test workspaces.
description: |
  The layout feature documents this repository's physical organization and
  maps implementation areas to their source paths. It is specific to the
  Bloomery repository and does not define a required layout for other Bloomery
  consumers.
---

# Repository Layout

This feature is a navigational contract for contributors: it explains where
this repository's binaries, libraries, Nix implementation, checks, tests, and
Bloomery specifications live.

## Design documents

- [Physical source layout](design/repository-layout.md)
- [Rust module architecture](design/architecture.md)
- [Tooling and dependency roles](design/tooling.md)

## Scope

This feature describes path ownership and package organization. It does not
replace the cross-repository `.bloomery/specs` document contract specified by
[PARSER/WORKSPACE](../../PARSER/WORKSPACE/README.md), nor does it own behavior
contracts for the CLI or documentation site.
