# Derivation contracts

## Crate compilation

A crate derivation invokes `rustc` directly. It builds an `rlib` for libraries
and a dynamic library for proc macros, and also emits manifest-declared
`cdylib` and `staticlib` crate types. External crates compile with lints
capped, and compiler metadata and extra filename hashes derive from the package
ID and active features. The builder test suite compiles a proc-macro fixture
crate and asserts its dynamic-library crate type, keeping proc-macro support
covered independently of evidence annotations.

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
`DEP_<LINKS>_<KEY>` for dependent build scripts. Native link inputs produced by
build scripts are installed and propagated as described in
[Build-script native artifacts](native-artifacts.md).

## Feature compilation

Active features become `--cfg feature="..."` flags. Features implied through a
crate's `[features]` table are expanded, and features that only reference
optional dependencies absent from the graph are skipped. With non-unified
resolution a package reachable from more than one context produces one crate
derivation per context. Each context variant carries a distinct deterministic
identifier derived from the context and the package ID; that identifier
qualifies its graph node key and derivation name. A package resolved by a single
context keeps its package-based identifier.

## Binary derivations

Workspace binaries are separate derivations that link the member's library
crate when present plus its dependency closure. Binaries are discovered from
manifest `[[bin]]` entries, implicit `src/main.rs`, `src/bin/*.rs`, and
`src/bin/*/main.rs`, with manifest entries winning duplicates. Release and dev
binaries share discovery but use their respective profile flags and crate
variants. The platform default linker is used unless the toolchain or profile
selects one.

## Runtime dependencies

A binary override may declare runtime tool dependencies that the executable
invokes as subprocesses. The installed executable is wrapped so the declared
tools are prepended to its runtime `PATH`, and the wrapped derivation references
those tools in its runtime closure. Runtime dependencies do not change the
binary's compilation inputs.

## Assets

Crate derivations install local `assets`, `static`, and `public` directories
plus configured asset directories, and forward dependency assets. Binary
derivations bundle dependency, workspace, and crate-local assets into
`$out/bin/<dir>` and `$out/share/<bin>/<dir>`; explicit override assets are
bundled under `assets/`. Crate-local assets take precedence over dependency
assets.

## Compressed crate artifacts

A crate derivation installs its compiled libraries, compiler metadata,
proc-macro dynamic libraries, and any build-script native libraries as a single
zstd-compressed archive named `lib.tar.zst` instead of an uncompressed `lib/`
directory. The archive is built from the crate's library directory so
extraction restores the installed layout. The crate metadata script publishes
the archive path and the primary library's filename in place of an absolute
library path.

Every Rust builder extracts each direct dependency's archive into its
build-local dependency directory before invoking `rustc`, then links the
extracted primary library with `--extern`. A crate publishes its own archive
together with the archives inherited from its dependencies in `deps-closure`,
so a consumer can restore the transitive dependency set. Ordinary library
directories remain supported for externally supplied dependencies.

Archiving is an installation detail: crate, binary, test, documentation, and
clippy outcomes are unchanged.

## Checks

`<crate>:test` compiles and runs unit tests and integration tests with
`rustc --test` and test overrides. `<crate>:clippy` runs `clippy-driver` with
warnings denied by default. `<crate>:doc` builds rustdoc output.
`<crate>:doctest` runs rustdoc tests for library crates. Package checks reuse
the release crate and binary derivations. `workspace:lock` is a read-only
derivation that validates `Cargo.lock` and `bloomery.lock` consistency, member
presence, versions, and dependency completeness, and reports `bloomery sync`
as the repair.

## Lock validation

Member metadata used by `workspace:lock` is resolved through Cargo's package
field inheritance: when a member sets a `[package]` field such as `version` to
`{ workspace = true }`, the check uses the value from the root
`[workspace.package]` table. The resolved version is compared with the member's
`Cargo.lock` entry and appears in any mismatch report, so an inherited version
that matches passes and a mismatch reports readable versions.