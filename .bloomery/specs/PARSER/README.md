---
id: PARSER
name: Bloomery Parsing and Evidence Area
tagline: Resolve repository specifications and statically extract requirement evidence.
description: |
  The PARSER area defines the shared input contracts used by Bloomery
  commands: configuration, grouped requirements, workspace documents, and
  source-evidence references. It resolves these inputs into a common model while
  retaining stable identities, source locations, and structured diagnostics.
---

# PARSER Area

PARSER owns the reusable ingestion path shared by `bloomery check` and
`bloomery review`. It interprets configuration and specification documents,
constructs the resolved requirement model, and extracts evidence from configured
source inputs. Command-level validation and review behavior remain in the
[CLI area](../CLI/README.md).

## Features

| Feature | Responsibility | Design |
| --- | --- | --- |
| [Configuration](CONFIGURATION/README.md) | Config defaults, validation, and scanner targets | [config file](CONFIGURATION/design/config-file.md), [scanner targets](CONFIGURATION/design/scanner-targets.md) |
| [Requirements](REQUIREMENTS/README.md) | EARS record format, identifiers, and domain model | [format](REQUIREMENTS/design/format.md), [identifiers](REQUIREMENTS/design/identifiers.md), [domain model](REQUIREMENTS/design/domain-model.md) |
| [Workspace](WORKSPACE/README.md) | Specification discovery, document contracts, and design links | [layout](WORKSPACE/design/layout.md), [document contracts](WORKSPACE/design/document-contracts.md) |
| [Scanning](SCANNING/README.md) | Static evidence extraction from Rust, Playwright, and Nix | [overview](SCANNING/design/overview.md), [Rust](SCANNING/design/rust.md), [Playwright](SCANNING/design/playwright.md), [Nix](SCANNING/design/nix.md) |

## Implementation boundaries

The reference Rust implementation keeps command dispatch, parsing, domain
modeling, scanners, validation, and output separable. A single resolved graph is
shared by check and review. Source-language parsers sit behind scanner
boundaries, and source locations and structured diagnostics are preserved
through to presentation. The Nix adapter is likewise isolated behind a scanner
boundary so metadata evaluation does not introduce build or test execution into
CLI verification.

These are reference implementation boundaries subordinate to the command and
feature contracts; implementation details may evolve without changing the
workspace or traceability model. The repository-specific crate map and
dependency roles are recorded in
[REPOSITORY/LAYOUT](../REPOSITORY/LAYOUT/README.md).