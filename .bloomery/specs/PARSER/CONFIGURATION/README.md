---
id: CONFIGURATION
name: Bloomery Configuration
tagline: Required roots, scanner paths, and parser options.
description: |
  Configuration controls where requirements are discovered and which source
  trees are inspected for static references and test sites. It selects scanner
  behavior without changing requirement identity or the architecture model. It
  also resolves whether specification management is active so build and check
  can run without a specification tree.
---

# Configuration

Bloomery reads `.bloomery/config.toml` from the repository root. The file is
required for every repository command except parser help, `init`, and `config`.
Configuration is declarative input to discovery and scanning; it is not a second
source of requirement identity or lifecycle state. The file also carries the
build tables and the sub-flake composition table consumed by
[NIXLIB/FLAKE](../../NIXLIB/FLAKE/README.md), which the parser type-validates
during loading and passes through for the flake interface. The top-level
integer `version` identifies the structural layout; format `1` is currently the
only supported version. A missing or unsupported version is rejected before
configuration validation.

## Design documents

- [Configuration file](design/config-file.md)
- [Schema catalog](design/schema-catalog.md)
- [Code health policy](design/code-health.md)
- [Clippy suppression policy](design/clippy-suppressions.md)
- [Scanner targets](design/scanner-targets.md)

## Responsibilities

- Require `.bloomery/config.toml` for repository commands while exempting help,
  `init`, and `config`, whose mutating commands create a missing file with a
  warning.
- Require the top-level format version, support format `1`, and normalize the
  configuration before validation.
- Keep configuration format versions stable when the schema catalog gains keys.
- Use validated defaults for tables and keys omitted from a present file,
  including enabling every supported scanner by default.
- Locate the specs root relative to `.bloomery/`.
- Resolve specification management from an optional enable override and the
  presence of the specification tree.
- Enable or disable supported scanners.
- Define source globs relative to the repository root.
- Define Playwright tag prefixes.
- Define repository-relative Nix source paths for static evidence scanning.
- Define repository-relative Nix test globs for candidate discovery.
- Accept the build tables without interpreting them as scanner or spec
  configuration while cataloguing and type-validating their keys.
- Catalogue the parameterized formatter enablement and ordering keys alongside
  the build tables.
- Catalogue the parameterized sub-flake composition table and its per-flake
  elevation keys alongside the build tables.
- Expose a schema catalog spanning every configurable option except format
  metadata, with documentation text and recommended defaults, consumed by
  `bloomery config` and sync recommendations.
- Resolve and validate CLI-owned `checks.codeHealth` scoring and baseline policy.
- Resolve and validate CLI-owned `checks.clippySuppressions` enablement and exception names.
- Resolve and validate CLI-owned `checks.specifications` check enablement.
- Reject malformed or ambiguous configuration before scanning begins.

## Dependencies

[PARSER/WORKSPACE](../WORKSPACE/README.md) consumes the resolved
specification-management state and the specs root; when management is disabled
it produces an empty workspace. [PARSER/SCANNING](../SCANNING/README.md) consumes scanner paths and
source-specific options; Nix evidence is read statically from source metadata. [CLI/CHECK](../../CLI/CHECK/README.md) treats
configuration errors as an early failure and does not proceed with an
incomplete configuration. [CLI/CONFIG](../../CLI/CONFIG/README.md) edits every
catalogued table, including the build tables, through the schema and its
recommended-default catalog. The
[NIXLIB/FLAKE composition table](../../NIXLIB/FLAKE/design/composition.md) is
catalogued here and evaluated by `mkFlake`.
