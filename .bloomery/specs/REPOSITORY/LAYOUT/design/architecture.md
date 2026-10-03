# Rust crate architecture

Rust binaries are entry-point wrappers. Application behavior lives in library
crates, organized by ownership category:

```text
packages/rust/
├── bins/
│   ├── cli/                    # delegates to cli-app
│   └── docs/                   # delegates to docs-server
└── libs/
    ├── shared/
    │   ├── model/              # domain types, configuration, diagnostics
    │   ├── workspace/          # filesystem, Markdown, and TOML loading
    │   ├── scanning/           # Rust, Playwright, and Nix evidence scanners
    │   ├── check/              # evidence and traceability validation
    │   └── test-macros/         # test-only evidence annotations
    ├── cli/
    │   ├── cli-types/          # parsed command requests, independent of Clap
    │   ├── cli-parser/         # Clap syntax and request conversion
    │   ├── cli-app/            # process-level dispatch and command coordination
    │   ├── cli-output/         # shared terminal and JSON formatting
    │   ├── check-command/      # Nix catalog, execution, storage, and retrieval
    │   ├── review/             # review catalogue and projection
    │   └── sync/               # lockfile synchronization workflow
    └── docs/
        ├── content/            # documentation catalog and source content
        ├── ui/                 # pages and components
        └── docs-server/        # server configuration, routing, assets, and listener
```

The dependency direction is one-way:

```text
bins/cli ──► cli-app ──► cli-parser ──► cli-types
                 ├────► check-command ──► check, workspace, model
                 │                         └──────────────► cli-output
                 ├────► sync, review, workspace, model
                 └────► cli-output

bins/docs ──► docs-server ──► ui ──► content

check ──► scanning ──► model
```

The parser produces Clap-independent request data; command libraries do not
parse process arguments. The app library coordinates command crates, while each
command crate owns its domain operations and exposes injectable dependencies
for testing.

The Nix scanner is static: it reads source metadata and does not evaluate Nix.
The docs server library owns environment configuration, router setup, asset
loading, listener binding, and serving. The documentation UI and its content
catalog remain separate from the server lifecycle.
