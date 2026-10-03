---
id: CONFIGURATION
name: Bloomery Configuration
tagline: Explicit roots, scanner paths, parser options, and Nix evaluation targets.
description: |
  Configuration controls where requirements are discovered and which source
  trees are inspected for static references. It selects scanner behavior and
  Nix systems without changing requirement identity or the architecture model.
---

# Configuration

Bloomery reads `.bloomery/config.toml` from the repository root. Configuration
is declarative input to discovery and scanning; it is not a second source of
requirement identity or lifecycle state.

## Design documents

- [Configuration file](design/config-file.md)
- [Scanner targets](design/scanner-targets.md)

## Responsibilities

- Use validated defaults when `.bloomery/config.toml` is absent or omits tables
  and keys.
- Locate the specs root relative to `.bloomery/`.
- Enable or disable supported scanners.
- Define source globs relative to the repository root.
- Define Playwright tag prefixes.
- Define the Nix checks attribute and target systems.
- Reject malformed or ambiguous configuration before scanning begins.

## Dependencies

[PARSER/WORKSPACE](../WORKSPACE/README.md) consumes the specs root.
[PARSER/SCANNING](../SCANNING/README.md) consumes scanner paths and
source-specific options. [CLI/CHECK](../../CLI/CHECK/README.md) treats
configuration errors as an early failure and does not proceed with an
incomplete configuration.
