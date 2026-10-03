---
id: REQUIREMENTS
name: Bloomery Requirements Model
tagline: Grouped EARS records with stable identities and explicit verification modes.
description: |
  The requirements feature defines the TOML record format, EARS clause
  grammar, manual-versus-automated verification boundary, and identifiers that
  align with service, feature, and requirement-group paths. It specifies the
  domain model consumed by check and review.
---

# Requirements

Requirement records live in grouped TOML files below a feature's
`requirements/` directory. The model is shared by parsing, check, and review.

## Design documents

- [Grouped format and EARS grammar](design/format.md)
- [Identifiers and filesystem alignment](design/identifiers.md)
- [Resolved domain model](design/domain-model.md)

## Responsibilities

- Define the fields and structural grammar of a requirement record.
- Generate a canonical EARS statement from typed clause fields.
- Separate automated verification (`manual = false`) from human review
  (`manual = true`).
- Bind service, feature, group, and sequence identity to physical paths.
- Provide one resolved requirement model to check and review.

## Dependencies

[PARSER/WORKSPACE](../WORKSPACE/README.md) supplies service and feature
identity and resolves design links. [PARSER/SCANNING](../SCANNING/README.md)
produces references to automated records. The CLI review command selects manual
records from this model.
