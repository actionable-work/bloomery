# Benchmark fixtures

## Layout

```text
benchmarks/
├── flake.nix                       # benchmark subflake: the bench app
├── run.sh                          # hyperfine entry point
├── harness/                        # mutation, preparation, and report helpers
├── fixtures/
│   ├── simple/                     # small binary + library, one registry dependency
│   └── standard/                   # multi-member workspace with axum and clap
├── results/
│   ├── current.md                  # regenerated snapshot of the latest run
│   └── history.json                # appended run history over time
└── flakes/
    ├── bloomery/flake.nix
    ├── crane/flake.nix
    ├── cargo2nix/flake.nix
    ├── crate2nix/flake.nix
    ├── naersk/flake.nix
    └── rustPlatform/flake.nix
```

Each subflake is a self-contained flake. It pins its builder input and takes the
fixture workspace as a `workspace` path input. The harness overrides that input
per fixture with `--override-input workspace path:benchmarks/fixtures/<group>`,
so every builder compiles the same bytes for a given group.

## Fixture groups

Two groups are measured separately:

| Group | Contents |
| --- | --- |
| `simple` | `app` binary and `util` library; `app` depends on `util`; one crates.io dependency (`itoa`) |
| `standard` | `app`, `server`, `shared`, `leaf`, `util`, `macros`, and `build` members; `server` uses `axum` and `app` uses `clap` |

The standard group is a Cargo workspace with a stable dependency graph:

| Member | Kind | Role |
| --- | --- | --- |
| `app` | bin | default build target; depends on `clap`, `server`, `shared`, and `leaf` |
| `server` | lib | depends on `axum`, `shared`, and `leaf` |
| `shared` | lib | depends on `util` |
| `leaf` | lib | depends on `util` and `macros`; carries a doctest |
| `util` | lib | internal dependency consumed by `shared` and `leaf`; carries unit tests |
| `macros` | proc-macro | consumed by `leaf` to exercise the proc-macro path |
| `build` | lib | carries a `build.rs` build script |

Both groups declare their crates.io dependencies in
`[workspace.dependencies]`, include a member with unit tests, and set the
release profile so the fixture matches the benchmark profile. Both expose
`crates/app` and `crates/util`, so the scenario targets are fixture-agnostic.

## Builder contract

Every builder subflake exposes, for each selected system:

- `packages.<system>.default`, the built `app` binary; and
- `checks.<system>.*`, the builder's idiomatic check derivations.

The harness addresses only these attributes, so the same commands run against
every builder and fixture. The default system set is the benchmark system set,
narrowed to one target platform for measurement.

## Check isolation

The `benchmarks/` directory is a separate flake. It is not declared in the root
`.bloomery/config.toml` `[flakes]` table, and the root flake does not reference
it, so benchmark packages, apps, and checks do not contribute to the root flake
outputs. Adding an alternative input therefore does not change root flake
evaluation, and `bloomery check` does not compile benchmark fixtures. The
benchmark subflake exposes the `bench` runner app; the root flake exposes a
second `bench` runner app.

## Validation

A structural check under `nix/checks/` evaluates the benchmark subflakes and
asserts the builder contract: fixture input, `packages.default`, non-empty
`checks`, pinned inputs, both fixture groups, and shared scenario targets. It
does not run timed measurements and is the automated evidence for the fixture
and alternative requirements.