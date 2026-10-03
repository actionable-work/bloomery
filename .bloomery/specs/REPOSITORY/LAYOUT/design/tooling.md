# Tooling and dependency roles

The reference implementation uses focused Rust crates behind small adapters:

| Role | Expected tool |
| --- | --- |
| CLI parsing | `clap` |
| Serialization | `serde`, `serde_json`, `toml`, `serde_yaml` |
| Repository globbing | `globwalk` |
| Markdown parsing | `pulldown-cmark` |
| Rust AST parsing | `syn` |
| TypeScript/JavaScript AST parsing | `oxc_allocator`, `oxc_parser`, `oxc_span`, `oxc_ast` |
| Natural sequence ordering | `natord` |
| Error modeling | `thiserror` |
| Terminal presentation | `owo-colors` |

The dependency set is:

```toml
clap = { version = "4.5", features = ["derive", "cargo"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"
serde_yaml = "0.9"
globwalk = "0.9"
pulldown-cmark = "0.12"
syn = { version = "2.0", features = ["full", "extra-traits"] }
oxc_allocator = "0.30"
oxc_parser = "0.30"
oxc_span = "0.30"
oxc_ast = "0.30"
natord = "1.0"
thiserror = "2.0"
owo-colors = "4.1"
```

These are implementation guidance, not a new repository dependency change. The
static-only boundary remains the primary constraint: no dependency may turn
`check` into a test runner or build orchestrator.
