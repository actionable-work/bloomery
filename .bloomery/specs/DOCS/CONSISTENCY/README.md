---
id: CONSISTENCY
name: Documentation Consistency
tagline: Keep authored documentation and the README aligned with the living specification.
description: |
  The consistency feature owns the documentation coverage manifest and the
  static validation that authored documentation and the repository README stay
  aligned with the specification tree. It checks coverage, link resolution,
  and executable excerpts, and reports documentation drift as a repository-local
  static check.
---

# Documentation Consistency

Authored documentation drifts when the specification changes. This feature
makes documentation coverage and references explicit so drift is a check
failure instead of a silent omission.

## Responsibilities

- Own `.bloomery/docs.manifest.toml`, the documentation coverage manifest.
- Require every specification feature to be named by at least one documentation
  surface.
- Require every documentation surface to be registered and to exist.
- Resolve repository-relative documentation links and their anchors.
- Validate role-tagged configuration and command excerpts against the
  configuration schema catalog and the CLI command catalog.
- Report documentation drift through a repository-local check in the root
  flake's check suite.

## Design documents

- [Consistency validation](design/consistency.md)

## Boundaries

Consistency validation is static: it reads documentation, the coverage
manifest, and the specification tree, and never renders, serves, evaluates, or
executes documentation. It does not rewrite documentation, generate pages, or
certify prose accuracy. The check is specific to this repository and is not
exposed as a shared Bloomery CLI or specification feature. Page content,
rendering, and serving belong to [DOCS/CONTENT](../CONTENT/README.md),
[DOCS/UI](../UI/README.md), and [DOCS/SERVER](../SERVER/README.md).
