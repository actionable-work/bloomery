# Service and feature document contracts

Service and feature entry points are Markdown files with YAML frontmatter.
They are intentionally human-readable while exposing a small machine-readable
contract for discovery and CLI summaries.

## Required frontmatter

Every service and feature README contains these string keys:

| Key | Meaning |
| --- | --- |
| `id` | Exact identifier represented by the directory name. |
| `name` | Formal human-readable title. |
| `tagline` | One-sentence summary for compact CLI output. |
| `description` | Paragraph or block describing scope, boundaries, and architecture. |

The `id` is compared verbatim with its directory. The other fields are content,
not alternate identity sources.

## Service README

A service README describes the domain boundary, ownership, major invariants,
and the features below it. It should explain what the service does and what it
deliberately leaves to external systems.

## Feature README

A feature README describes one cohesive capability. It should state the
capability's scope, dependencies, and design documents. A requirement with no
explicit design path resolves to this README, so it must be a meaningful
architectural overview rather than a directory marker.

## Design links

Feature READMEs provide relative links to their `design/` documents. A
requirement may refer to a document directly, optionally with a heading anchor.
The checker validates both the file and the anchor when a link is explicit.

## Living-document invariant

Architecture is committed alongside the records and code it explains. Bloomery
does not accept a requirement as a detached list item with no service or
feature context. Lifecycle state belongs to Git branches and pull requests,
not to frontmatter.
