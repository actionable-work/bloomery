---
id: CONFIGURATION
name: Bloomery Configuration
tagline: Required roots, scanner paths, and parser options.
description: |
  Configuration controls where requirements are discovered and which source
  trees are inspected for static references. It selects scanner behavior
  without changing requirement identity or the architecture model.
---

# Configuration

Bloomery reads `.bloomery/config.toml` from the repository root. The file is
required for every repository command except parser help and `init`.
Configuration is declarative input to discovery and scanning; it is not a second
source of requirement identity or lifecycle state. The file also carries the
build tables consumed by [NIXLIB/FLAKE](../../NIXLIB/FLAKE/README.md), which the
parser type-validates during loading and passes through for the flake
interface.

## Design documents

- [Configuration file](design/config-file.md)
- [Schema catalog](design/schema-catalog.md)
- [Scanner targets](design/scanner-targets.md)

## Responsibilities

- Require `.bloomery/config.toml` for repository commands while exempting help,
  `init`, and `config`, whose mutating commands create a missing file with a
  warning.
- Use validated defaults for tables and keys omitted from a present file.
- Locate the specs root relative to `.bloomery/`.
- Enable or disable supported scanners.
- Define source globs relative to the repository root.
- Define Playwright tag prefixes.
- Define repository-relative Nix source paths for static evidence scanning.
- Accept the build tables without interpreting them as scanner or spec
  configuration while cataloguing and type-validating their keys.
- Expose a schema catalog spanning every recognized key, consumed by
  `bloomery config` and sync recommendations.
- Reject malformed or ambiguous configuration before scanning begins.

## Dependencies

[PARSER/WORKSPACE](../WORKSPACE/README.md) consumes the specs root.
[PARSER/SCANNING](../SCANNING/README.md) consumes scanner paths and
source-specific options; Nix evidence is read statically from source metadata. [CLI/CHECK](../../CLI/CHECK/README.md) treats
configuration errors as an early failure and does not proceed with an
incomplete configuration. [CLI/CONFIG](../../CLI/CONFIG/README.md) edits every
catalogued table, including the build tables, through the schema and its
recommended-default catalog.
