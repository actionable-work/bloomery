# Scanner architecture

The scanner stage runs after configuration and requirement parsing. Every
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

- Only configured source paths are scanned.
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

A single command parses a given source file at most once. Reference extraction
and test discovery share one command-scoped parse cache keyed by the canonical
repository-relative path, so a file matched by overlapping globs or used in both
roles yields one syntax tree. Repeated queries return the cached tree rather
than reparsing.

The cache is created at the start of a scanner pass and discarded when the
command finishes. It is never persisted, shared between command invocations, or
used to skip source reads that the command must perform. A parse failure is
reported once per file per command and contributes neither references nor test
sites.
