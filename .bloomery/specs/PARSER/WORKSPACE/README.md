---
id: WORKSPACE
name: Bloomery Specification Workspace
tagline: Discover service and feature documents and resolve their design and requirement paths.
description: |
  The workspace feature defines how Bloomery finds its configuration and
  specification tree, pairs service and feature documents with directories,
  and resolves relative design references. It provides the filesystem contract
  consumed by the other PARSER features.
---

# Specification Workspace

The workspace is the repository-local `.bloomery/` directory. It is the source
of truth for Bloomery configuration and architecture metadata; source-code tests
remain in their normal repository locations and are found through
[PARSER/CONFIGURATION](../CONFIGURATION/README.md).

## Design documents

- [Workspace layout](design/layout.md)
- [Service and feature document contracts](design/document-contracts.md)

## Responsibilities

- Establish the `.bloomery/config.toml` and `.bloomery/specs/` roots.
- Discover service and feature directories without case normalization.
- Require a service `README.md` and a feature `README.md` when those nodes
  exist in the design tree.
- Require every feature to contain a `requirements/` directory with at least
  one grouped TOML file.
- Resolve feature-local design links and optional Markdown anchors.

## Dependencies

[PARSER/CONFIGURATION](../CONFIGURATION/README.md) supplies the roots and
scanner settings. [PARSER/REQUIREMENTS](../REQUIREMENTS/README.md) defines the
grouped TOML records owned by each feature. [CLI/CHECK](../../CLI/CHECK/README.md)
consumes the resolved workspace graph and reports missing documents or broken
links.
