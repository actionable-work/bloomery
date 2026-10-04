# Physical repository layout

Bloomery is a Rust workspace and a Nix library. Its top-level layout separates
Rust binaries and libraries, reusable Nix implementation, flake checks, test
workspaces, and repository-local Bloomery metadata.

| Path | Purpose |
| --- | --- |
| `Cargo.toml`, `Cargo.lock` | Rust workspace membership and locked dependencies. |
| `packages/rust/bins/` | Thin executable entry points. |
| `packages/rust/libs/shared/` | Library crates shared across application layers. |
| `packages/rust/libs/cli/` | CLI parsing, application, command, and presentation libraries. |
| `packages/rust/libs/docs/` | Documentation content, UI, and server libraries. |
| `lib/` | Reusable Nix library: builders, workspace discovery, profiles, overrides, locks, docs, and modules. |
| `nix/checks/` | Flake checks, including documentation asset validation and workspace checks. |
| `nix/lib/` | Shared Nix helpers used by the flake. |
| `tests/` | Isolated consumer workspaces, Nix integration fixtures, and test support. |
| `.bloomery/config.toml` | Optional local scanner configuration for this repository. |
| `.bloomery/specs/` | Bloomery area and feature specifications for this repository. |

The Rust workspace contains these binaries and libraries:

| Package path | Package | Spec ownership |
| --- | --- | --- |
| `packages/rust/bins/cli` | `bloomery-cli` | `CLI/INTERFACE` |
| `packages/rust/libs/cli/cli-app` | `bloomery-cli-app` | `CLI/INTERFACE` |
| `packages/rust/libs/cli/cli-parser` | `bloomery-cli-parser` | `CLI/INTERFACE` |
| `packages/rust/libs/cli/cli-types` | `bloomery-cli-types` | `CLI/INTERFACE` |
| `packages/rust/libs/cli/cli-output` | `bloomery-cli-output` | `CLI/INTERFACE` |
| `packages/rust/libs/cli/check-command` | `bloomery-check-command` | `CLI/CHECK` |
| `packages/rust/libs/shared/check` | `bloomery-check` | `CLI/CHECK` |
| `packages/rust/libs/cli/review` | `bloomery-review` | `CLI/REVIEW` |
| `packages/rust/libs/cli/sync` | `bloomery-sync` | `CLI/SYNC` |
| `packages/rust/libs/shared/model` | `bloomery-model` | `PARSER/CONFIGURATION`, `PARSER/REQUIREMENTS` |
| `packages/rust/libs/shared/workspace` | `bloomery-workspace` | `PARSER/WORKSPACE` |
| `packages/rust/libs/shared/scanning` | `bloomery-scanning` | `PARSER/SCANNING` |
| `packages/rust/libs/shared/test-macros` | `bloomery-test-macros` | Rust evidence annotations in `PARSER/SCANNING` |
| `packages/rust/bins/docs` | `bloomery-docs` | `DOCS/SERVER` |
| `packages/rust/libs/docs/docs-server` | `bloomery-docs-server` | `DOCS/SERVER` |
| `packages/rust/libs/docs/content` | `bloomery-content` | `DOCS/CONTENT` |
| `packages/rust/libs/docs/ui` | `bloomery-ui` | `DOCS/UI` |

The Nix workspace-check integration is described with
[CLI/CHECK](../../../CLI/CHECK/design/workspace-check.md). This layout map
records where that integration and the broader Nix library live; the detailed
contracts of those Nix subsystems remain outside this feature's scope.
[Source isolation](../../../NIX/SOURCES/README.md) owns fileset selection and
rebuild boundaries for the reusable builders and repository test-support inputs.
