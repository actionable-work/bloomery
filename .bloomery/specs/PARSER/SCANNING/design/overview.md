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

- Only configured paths and systems are scanned.
- Requirement IDs are extracted from syntax nodes or declared metadata, not
  from arbitrary comments or prose.
- Parser failures are reported with source context; the scanner does not fall
  back to regex matching after a syntax error.
- References are de-duplicated for set validation but retain all meaningful
  source locations for diagnostics.
- A reference to an unknown ID is not ignored; it becomes an orphan-reference
  error in `bloomery check`.

The supported source-specific contracts are described in the linked Rust,
Playwright, and Nix documents.
