---
id: WORKSPACE
name: Bloomery Workspace
tagline: Verbatim repository layout for services, features, designs, and requirement groups.
description: |
  The workspace feature defines how Bloomery finds its configuration and
  design tree, how service and feature documents are paired with directories,
  and how relative design references are resolved. It provides the stable
  filesystem contract used by every other CLI feature.
---

# Workspace

The workspace is the repository-local `.bloomery/` directory. It is the source
of truth for Bloomery configuration and architecture metadata; source-code
tests remain in their normal repository locations and are found through the
[configuration](../CONFIGURATION/README.md) feature.

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

Configuration supplies the roots and scanner settings. Requirements define the
grouped TOML records owned by each feature. Check consumes the resolved
workspace graph and reports missing documents or broken links.
