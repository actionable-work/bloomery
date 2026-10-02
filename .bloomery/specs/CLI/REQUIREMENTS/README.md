---
id: REQUIREMENTS
name: Bloomery Requirements
tagline: Grouped EARS records with stable identities and explicit verification modes.
description: |
  The requirements feature defines the TOML record format, EARS clause
  grammar, manual-versus-automated verification boundary, and four-segment
  identifiers that mirror the filesystem. It specifies the data consumed by
  check and review.
---

# Requirements

Requirement records live in grouped TOML files below a feature's
`requirements/` directory.

## Design documents

- [Grouped format and EARS grammar](design/format.md)
- [Identifiers and filesystem alignment](design/identifiers.md)

## Responsibilities

- Define the fields and structural grammar of a requirement record.
- Generate a canonical EARS statement from typed clause fields.
- Separate automated verification (`manual = false`) from human review
  (`manual = true`).
- Bind service, feature, group, and sequence identity to physical paths.
- Provide the set of declared requirements consumed by check and review.

## Dependencies

Workspace supplies service and feature identity. Design links provide the
architecture target for each record. Scanning produces references to automated
records; review selects manual records.
