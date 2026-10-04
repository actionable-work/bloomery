# Derivation contracts

## Crate compilation

A crate derivation invokes `rustc` directly. It builds an `rlib` for libraries
and a dynamic library for proc macros, and also emits manifest-declared
`cdylib` and `staticlib` crate types. External crates compile with lints
capped, and compiler metadata and extra filename hashes derive from the package
ID and active features.

## Dependency wiring and metadata

Direct dependencies are exposed with `--extern` from each dependency's
`nix-support/meta.sh`, including aliases declared with `package = ...`. Each
crate records its name, library path, proc-macro flag, link flags, assets, and
sys-crate `DEP_<LINKS>_<KEY>` values in that metadata, and publishes a
`deps-closure` of transitive libraries for downstream compilation.

## Build scripts

A crate or binary with a build script compiles and executes it before the
target. The script receives the `OUT_DIR`, `TARGET`, `HOST`, `NUM_JOBS`,
`OPT_LEVEL`, `PROFILE`, `CARGO_CFG_*`, and `CARGO_FEATURE_*` environment that
Cargo exposes. Both legacy `cargo:` and modern `cargo::` directives are parsed;
cfg, link-lib, link-search, rustc-env, and rustc-flags directives are applied
to the target compilation. A `links` crate publishes its metadata keys as
`DEP_<LINKS>_<KEY>` for dependent build scripts.

## Feature compilation

Active features become `--cfg feature="..."` flags. Features implied through a
crate's `[features]` table are expanded, and features that only reference
optional dependencies absent from the graph are skipped.

## Binary derivations

Workspace binaries are separate derivations that link the member's library
crate when present plus its dependency closure. Binaries are discovered from
manifest `[[bin]]` entries, implicit `src/main.rs`, `src/bin/*.rs`, and
`src/bin/*/main.rs`, with manifest entries winning duplicates. Release and dev
binaries share discovery but use their respective profile flags and crate
variants. The platform default linker is used unless the toolchain or profile
selects one.

## Assets

Crate derivations install local `assets`, `static`, and `public` directories
plus configured asset directories, and forward dependency assets. Binary
derivations bundle dependency, workspace, and crate-local assets into
`$out/bin/<dir>` and `$out/share/<bin>/<dir>`; explicit override assets are
bundled under `assets/`. Crate-local assets take precedence over dependency
assets.

## Checks

`<crate>:test` compiles and runs unit tests and integration tests with
`rustc --test` and test overrides. `<crate>:clippy` runs `clippy-driver` with
warnings denied by default. `<crate>:doc` builds rustdoc output.
`<crate>:doctest` runs rustdoc tests for library crates. Package checks reuse
the release crate and binary derivations. `workspace:lock` is a read-only
derivation that validates `Cargo.lock` and `bloomery.lock` consistency, member
presence, versions, and dependency completeness, and reports `bloomery sync`
as the repair.