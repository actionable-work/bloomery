---
id: REVIEW
name: Manual Review Catalog
tagline: Deterministic human-review output for requirements that are not automated.
description: |
  The review feature implements `bloomery review`, selecting requirements with
  `manual = true`, generating their canonical EARS statements, resolving their
  design references, and rendering a stable tree or JSON document. It does not
  record review outcomes or assign reviewers.
---

# `bloomery review`

Review is a read-only projection of the requirement model for human auditing
and downstream workflow systems.

## Design documents

- [Manual catalogue](design/manual-catalog.md)
- [Output formats](design/output.md)

## Responsibilities

- Filter to manual requirements.
- Sort by service, feature, group, and numeric sequence.
- Render the canonical EARS statement and design reference.
- Support human-readable tree output and machine-readable JSON.
- Return a deterministic empty result when no manual records exist.

## Non-responsibilities

Review does not mutate records, mark them complete, create tickets, track
sign-offs, or execute any verification activity.
