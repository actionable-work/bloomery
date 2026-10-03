# Scanner architecture

The scanner stage runs after configuration and requirement parsing. Every
source-specific scanner emits the same logical evidence record:

```text
RequirementId + SourceLocation + ScannerKind
```

The registry is then consumed by the relational checks:

```text
declared requirements ──┐
                         ├──► orphan, illegal-manual, and coverage checks
static references ───────┘
```

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
