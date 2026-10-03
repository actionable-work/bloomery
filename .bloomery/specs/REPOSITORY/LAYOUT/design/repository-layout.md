# Physical repository layout

Bloomery is a Rust workspace and a Nix library. Its top-level layout separates
Rust binaries and libraries, reusable Nix implementation, flake checks, test
workspaces, and repository-local Bloomery metadata.

| Path | Purpose |
| --- | --- |
| `Cargo.toml`, `Cargo.lock` | Rust workspace membership and locked dependencies. |
| `packages/rust/bins/cli` | The `bloomery` command-line executable. |
| `packages/rust/bins/docs` | The Bloomery documentation server executable. |
| `packages/rust/crates/` | Rust libraries supporting parsing, checking, review, and documentation. |
| `lib/` | Reusable Nix library: builders, workspace discovery, profiles, overrides, locks, docs, and modules. |
| `nix/checks/` | Flake checks, including documentation asset validation and workspace checks. |
| `nix/lib/` | Shared Nix helpers used by the flake. |
| `tests/` | Isolated consumer workspaces, Nix integration fixtures, and test support. |
| `.bloomery/config.toml` | Optional local scanner configuration for this repository. |
| `.bloomery/specs/` | Bloomery service and feature specifications for this repository. |

The Rust workspace contains these binaries and libraries:

| Package path | Package | Spec ownership |
| --- | --- | --- |
| `packages/rust/bins/cli` | `bloomery-cli` | `CLI/INTERFACE` |
| `packages/rust/crates/check` | `bloomery-check` | `CLI/CHECK` |
| `packages/rust/crates/review` | `bloomery-review` | `CLI/REVIEW` |
| `packages/rust/crates/model` | `bloomery-model` | `PARSER/CONFIGURATION`, `PARSER/REQUIREMENTS` |
| `packages/rust/crates/workspace` | `bloomery-workspace` | `PARSER/WORKSPACE` |
| `packages/rust/crates/scanning` | `bloomery-scanning` | `PARSER/SCANNING` |
| `packages/rust/crates/test-macros` | `bloomery-test-macros` | Rust evidence annotations in `PARSER/SCANNING` |
| `packages/rust/bins/docs` | `bloomery-docs` | `DOCS/SERVER` |
| `packages/rust/crates/content` | `bloomery-content` | `DOCS/CONTENT` |
| `packages/rust/crates/ui` | `bloomery-ui` | `DOCS/UI` |

The Nix workspace-check integration is described with
[CLI/CHECK](../../../CLI/CHECK/design/workspace-check.md). This layout map
records where that integration and the broader Nix library live; the detailed
contracts of those Nix subsystems remain outside this feature's scope.
