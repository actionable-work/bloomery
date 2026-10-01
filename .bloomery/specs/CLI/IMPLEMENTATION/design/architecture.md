# Reference module architecture

The CLI is a modular Rust application:

```text
bloomery/
├── Cargo.toml
└── src/
    ├── main.rs                 # clap entry point and command dispatch
    ├── config.rs               # .bloomery/config.toml
    ├── model/
    │   ├── frontmatter.rs      # service and feature metadata
    │   ├── requirement.rs      # grouped TOML and EARS records
    │   └── context.rs          # resolved workspace graph
    ├── parser/
    │   ├── workspace.rs        # filesystem walk and path validation
    │   ├── markdown.rs         # frontmatter and heading anchors
    │   ├── rust.rs             # syn visitor
    │   ├── typescript.rs       # oxc parser visitor
    │   └── nix.rs              # metadata evaluation adapter
    ├── engine/
    │   ├── check.rs            # structural and relational validation
    │   └── review.rs           # sort and output projections
    ├── diagnostics.rs          # structured errors and source spans
    └── util/
        └── sort.rs             # natural sequence ordering
```

Command dispatch parses options, loads the context, invokes the selected engine,
and formats results. Parsers return domain data or structured diagnostics rather
than printing directly. Engines are testable against an in-memory context and
scanner registry.

The Nix adapter is kept behind a scanner boundary so static metadata evaluation
cannot leak process or build concerns into the domain model.
