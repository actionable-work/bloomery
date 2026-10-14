# Scanner architecture

Evidence scanning runs after configuration and requirement parsing. Rust health
analysis requires configuration and production source discovery, independently
of requirements. Both use [shared Rust ingestion](health-analysis.md). Every
source-specific scanner emits the same logical evidence record:

```text
RequirementId + SourceLocation + ScannerKind
```

Scanners additionally emit discovered test sites that carry zero or more
requirement references. Test discovery shares the same parse pass as reference
extraction; see the [test discovery contract](test-discovery.md).

The reference registry is consumed by the relational checks:

```text
declared requirements ──┐
                         ├──► orphan, illegal-manual, and coverage checks
static references ───────┘
```

Discovered test sites feed the [spec candidate projection](../../../CLI/SPEC/design/trace.md).

## Common rules

- Evidence uses configured scanner paths; health uses discovered production targets.
- Requirement IDs are extracted from syntax nodes or literal metadata, not from
  arbitrary comments or prose.
- Rust and Playwright parse failures are reported with source context; those
  scanners do not fall back to text matching after a syntax error.
- References are de-duplicated for set validation but retain all meaningful
  source locations for diagnostics.
- A reference to an unknown ID is not ignored; it becomes an orphan-reference
  error in `bloomery check`.

The supported source-specific contracts are described in the linked Rust,
Playwright, and Nix documents.

## Parse-once

A single command reads and parses a given source file at most once. Reference
extraction, test discovery, and health share one command-scoped ingestion cache
keyed by the canonical repository-relative path. Overlapping inputs yield one
syntax tree and one traversal; later queries reuse derived facts rather than
reparsing. Trees can be discarded after their reusable facts are extracted.

The cache is created before admitting source-consuming tasks and discarded when
the command finishes. It is never persisted, shared between command invocations, or
used to skip source reads that the command must perform. A parse failure is
retained once per file per command with affected consumer IDs; it contributes
neither references, test sites, nor health facts. Consumers retain independent
outcomes and never publish partial health scores.
