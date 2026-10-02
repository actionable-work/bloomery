# Reference module architecture

The CLI is a workspace of focused Rust crates:

```text
bloomery/
├── Cargo.toml
└── packages/rust/
    ├── bins/cli/               # clap entry point and command dispatch
    └── crates/
        ├── model/              # domain types, configuration, diagnostics
        ├── workspace/          # filesystem, Markdown, and TOML loading
        ├── scanning/           # Rust, Playwright, and Nix evidence scanners
        ├── check/              # structural and relational validation
        └── review/             # sorting and output projections
```

`bloomery-cli` parses options, loads the context through `bloomery-workspace`,
invokes `bloomery-check` or `bloomery-review`, and formats results. Domain
crates return structured data and diagnostics rather than printing directly.
Each engine is testable against an in-memory context and scanner registry.

The Nix adapter is kept behind a scanner boundary so static metadata evaluation
cannot leak process or build concerns into the domain model.
