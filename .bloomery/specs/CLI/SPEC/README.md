---
id: SPEC
name: Specification Record Management and Traceability
tagline: CRUD commands over requirement records plus test traceability and candidate discovery.
description: |
  The spec feature owns the `bloomery spec` command tree. It creates, reads,
  updates, and removes grouped requirement records in the repository
  specification tree, projects the tests statically tied to a requirement, and
  surfaces statically discovered tests that are not tied to any requirement as
  graduation candidates.
---

# `bloomery spec`

`bloomery spec` is the CLI surface for managing requirement records and
inspecting their relationship to automated tests. It composes the
[PARSER area](../../PARSER/README.md) workspace, requirement, and scanning
contracts with a typesafe editing and read-only projection layer.

## Design documents

- [Record editing](design/editing.md)
- [Traceability projections](design/trace.md)

## Responsibilities

- Expose `spec list`, `spec show`, `spec add`, `spec set`, `spec remove`,
  `spec trace`, and `spec candidates`.
- Address records by their stable requirement ID and derive their physical
  location from the identifier.
- Validate identifiers, EARS fields, and design references before writing.
- Preserve unrelated records, comments, and formatting.
- Publish mutations atomically.
- Report the tests statically tied to each requested requirement.
- Report statically discovered tests that carry no requirement reference.
- Support human-readable and JSON output.
- Return stable exit codes.

## Boundaries

Spec mutations edit grouped TOML requirement records below
`.bloomery/<specs.dir>/`. They never edit area or feature READMEs, design
documents, configuration, or lockfiles. Trace and candidate discovery read
source configured through `scanners.*` and never execute tests, launch
applications, or instantiate derivations. Spec does not run checks, manage
review status, or assign owners.