# Rust crate architecture

Rust binaries are entry-point wrappers. Application behavior lives in library
crates, organized by ownership category:

```text
packages/rust/
├── bins/
│   ├── cli/                    # delegates to cli-app
│   ├── docs/                   # delegates to docs-server
│   └── docs-check/             # delegates to docs consistency
└── libs/
    ├── shared/
    │   ├── schema/             # configuration schema, policies, requirement IDs, portable diagnostics
    │   ├── model/              # domain types, catalogue order, configuration, diagnostics
    │   ├── glob/               # anchored `*`/`?` matching, prefix feasibility, `**` paths
    │   ├── workspace/          # source trees, static Markdown and TOML loading
    │   ├── scanning/           # Rust, Playwright, and Nix evidence scanners
    │   ├── check/              # evidence, traceability, and clippy suppression validation
    │   └── test-macros/        # proc-macro build-graph fixture
    ├── cli/
    │   ├── cli-types/          # command requests and names, and the root-flake preflight
    │   ├── cli-parser/         # Clap syntax and request conversion
    │   ├── cli-app/            # dispatch plus per-command JSON and text presentation
    │   ├── cli-output/         # shared color, styling, and JSON envelope primitives
    │   ├── nix/                # Nix system names, store paths, evaluation results, CLI conventions
    │   ├── check-support/      # check model and statuses, context, progress, scheduling, private files
    │   ├── check-catalog/      # Nix check identifiers, discovery, planning, and realization
    │   ├── check-selection/    # selector matching and system resolution over the catalog
    │   ├── check-storage/      # run records, retained reports, and run loading
    │   ├── check-output/       # check rendering and JSON documents
    │   ├── check-retrieval/    # list, failures, and details over stored runs
    │   ├── check-health/       # health policy, reports, and destinations
    │   ├── check-command/      # check command orchestration
    │   ├── build-command/      # production package materialization
    │   ├── config/             # configuration editing workflows
    │   ├── disk/               # Nix store space measurement
    │   ├── init/               # template scaffolding
    │   ├── review/             # review loading, catalogue, and projection
    │   ├── spec/               # specification management workflows
    │   └── sync/               # lockfile synchronization workflow
    └── docs/
        ├── content/            # documentation catalog and source content
        ├── ui/                 # pages and components
        ├── docs-server/        # server configuration, routing, assets, and listener
        └── consistency/        # documentation coverage, link, and excerpt checks
```

The dependency direction is one-way:

```text
bins/cli ──► cli-app ──► cli-parser ──► cli-types
                 ├────► build-command ──► check-command
                 ├────► check-command ──► check-* ──► check, workspace, model
                 ├────► config, disk, init, review, spec, sync
                 └────► cli-output ──► cli-types, model

check-*, build-command, disk, cli-parser ──► nix
check-* ──► cli-output

bins/docs ──► docs-server ──► ui ──► content
bins/docs-check ──► consistency ──► cli-types, workspace, glob, model

check ──► scanning ──► workspace ──► glob, model ──► schema
```

The parser produces Clap-independent request data; command libraries do not
parse process arguments. The app library coordinates command crates and owns
their process-level JSON and text presentation, except that check and build
render their own streamed reports, while each command crate owns
its domain operations and exposes injectable dependencies for testing. The
shared output library carries only presentation primitives, so it depends on no
command crate. Each behavior shared across crates, such as glob matching, host
Nix system detection, Markdown anchors, command names, requirement ID grammar
and ordering,
and the root-flake preflight, has one
implementation in the lowest crate that every consumer can depend on.

The Nix scanner is static: it reads source metadata and does not evaluate Nix.
Workspace and scanning share repository-tree inputs. Specification discovery is
static over repository files; hosted lifecycle management changes those files.
The docs server library owns environment configuration, router setup, asset
loading, listener binding, and serving. The documentation UI and its content
catalog remain separate from the server lifecycle.
