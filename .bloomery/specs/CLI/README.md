---
id: CLI
name: Bloomery CLI Service
tagline: Offline static traceability from EARS requirements to architecture and tests.
description: |
  The CLI service discovers a Bloomery workspace, validates its living
  architecture and requirement records, extracts requirement references from
  configured source formats, and reports traceability failures deterministically.
  It also provides a read-only catalog of requirements that need human review.
  The service never runs test suites, application binaries, build derivations,
  or network operations as part of verification.
---

# Bloomery CLI Service

Bloomery is a repository-local service for maintaining a strict relationship
between requirements, architecture, and verification evidence. The service is
designed around two commands:

- `bloomery check` validates structure, grammar, source references, and
  automated-test coverage.
- `bloomery review` renders the manual requirements that need human attention.

## Boundaries

Bloomery owns document discovery, parsing, identifier alignment, static source
scanning, relational validation, and deterministic output. The `bloomery check`
command itself does not own requirement workflow, approval, test execution,
build orchestration, ticket management, or review scheduling. When a Rust
workspace has a `.bloomery/` directory, the Nix workspace builder invokes the
Bloomery package from the input flake as a generated check; that integration is
documented with the [implementation architecture](IMPLEMENTATION/README.md).

## Features

| Feature | Responsibility | Design |
| --- | --- | --- |
| [Workspace](WORKSPACE/README.md) | Repository layout and living document contracts | [layout](WORKSPACE/design/layout.md), [document contracts](WORKSPACE/design/document-contracts.md) |
| [Configuration](CONFIGURATION/README.md) | Discovery roots, scanner paths, and Nix targets | [config file](CONFIGURATION/design/config-file.md), [scanner targets](CONFIGURATION/design/scanner-targets.md) |
| [Requirements](REQUIREMENTS/README.md) | Grouped EARS records and identifier grammar | [format](REQUIREMENTS/design/format.md), [identifiers](REQUIREMENTS/design/identifiers.md) |
| [Scanning](SCANNING/README.md) | Static references from Rust, Playwright, and Nix | [overview](SCANNING/design/overview.md), [Rust](SCANNING/design/rust.md), [Playwright](SCANNING/design/playwright.md), [Nix](SCANNING/design/nix.md) |
| [Check](CHECK/README.md) | Structural, grammar, and coverage validation | [pipeline](CHECK/design/pipeline.md), [diagnostics](CHECK/design/diagnostics.md) |
| [Review](REVIEW/README.md) | Deterministic manual-review catalogues | [catalog](REVIEW/design/manual-catalog.md), [output](REVIEW/design/output.md) |
| [Implementation](IMPLEMENTATION/README.md) | Rust CLI architecture and Nix workspace-check integration | [architecture](IMPLEMENTATION/design/architecture.md), [model](IMPLEMENTATION/design/domain-model.md), [tooling](IMPLEMENTATION/design/tooling.md), [Nix workspace check](IMPLEMENTATION/design/workspace-check.md) |

## System flow

```text
workspace files
      │
      ├── configuration ──► discovery roots and scanner targets
      │
      ├── service/feature docs and requirement records
      │                         │
      │                         ▼
      │                   resolved requirement set
      │                         │
      └── Rust / TypeScript / Nix static scanners
                                │
                                ▼
                       reference registry
                                │
                 ┌──────────────┴──────────────┐
                 ▼                             ▼
          `bloomery check`              `bloomery review`
       validation and diagnostics     manual requirements
```

## Model invariant

Every feature has at least one grouped requirement file. Requirement records are
validated according to the [requirements design](REQUIREMENTS/README.md),
associated with living architecture, and consumed by both CLI commands.
