---
id: SYNC
name: Workspace Synchronization
tagline: Synchronize Cargo and Bloomery locks, optionally update dependencies, and surface unconfigured recommendations.
description: |
  The sync feature owns the mutating `bloomery sync` command. It reconciles
  Cargo.lock with the workspace manifests, regenerates bloomery.lock, optionally
  updates Rust dependencies and Nix flake inputs, and reports recommended
  configuration keys that the user has not explicitly configured.
---

# `bloomery sync`

Sync is the workspace maintenance command, not a traceability check. It replaces
Bloomery's Nix `lock` app with a CLI command; there is no replacement Nix `sync`
app. Its design and requirements define the implemented CLI behavior.

## Design documents

- [Synchronization design](design/sync.md)

## Responsibilities

- Preserve existing Cargo dependency versions where possible during normal sync.
- Offer explicit dependency updates through `--update[=nix,rust]`.
- Generate a compatible `bloomery.lock` from the final Cargo resolution.
- Report missing recommended keys using the raw configuration, not defaults.
- Warn non-fatally when `.bloomery/config.toml` is absent.
- Replace Nix lock-app exposure and lock-repair guidance with the CLI workflow.

## Boundaries and dependencies

Sync uses Cargo for dependency resolution and Nix only for selected flake-input
updates. It shares configuration syntax and validation with
[PARSER/CONFIGURATION](../../PARSER/CONFIGURATION/README.md), but does not load
specifications, scan evidence, execute checks, or require a `.bloomery/specs`
tree. Command syntax belongs to [INTERFACE](../INTERFACE/README.md).

Recommendations are advisory. Sync neither enables features nor writes config,
manifests, specifications, or source code. The recommendation catalog is built
into Bloomery and can grow over time without storing notification history.

## Verification

All requirements in this feature are automated (`manual = false`) and covered
by unit tests using temporary workspaces and injectable tool runners. The sync
command is also exercised directly against the repository during development.
