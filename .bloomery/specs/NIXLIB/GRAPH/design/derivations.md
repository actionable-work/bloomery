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

Direct dependencies are exposed with `--extern <edge-name>=<artifact-path>`
using resolved edge names and each dependency's `nix-support/meta.sh`, following
[Library names and dependency aliases](library-names.md). Each
crate records its name, library path, proc-macro flag, link flags, assets, and
sys-crate `DEP_<LINKS>_<KEY>` values in that metadata, and publishes a
`deps-closure` of the crates downstream compilation stages, as described in
[Crate archives and dependency staging](crate-archives.md).

## Build scripts

A crate or binary with a build script compiles and executes it before the
target. The script receives the `OUT_DIR`, `TARGET`, `HOST`, `NUM_JOBS`,
`OPT_LEVEL`, `PROFILE`, `CARGO_CFG_*`, and `CARGO_FEATURE_*` environment that
Cargo exposes. The crate builder also sets `CARGO_MANIFEST_LINKS` to the
manifest's `links` value when that key is present. A build script that exits
unsuccessfully fails the derivation and does not fall through to compilation,
and the script's captured output is reported in the build log. Both legacy
`cargo:` and modern `cargo::` directives are parsed; cfg, link-lib, link-search,
rustc-env, and rustc-flags directives are applied to the target compilation. A
`links` crate publishes its metadata keys as `DEP_<LINKS>_<KEY>` for dependent
build scripts. Native link inputs produced by build scripts are installed and
propagated as described in [Build-script native artifacts](native-artifacts.md).

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

Optimized binary variants enabled through `[optimize]` reuse the same binary
discovery, resolved dependency closure, selected sources, toolchain, and
profile. The [optimization pipeline](../../OPTIMIZE/design/optimization.md)
adds training and profiling stages around that compilation and exports the
result in place of the release package.

## Linking

Every step that links uses the selected linker: binaries, unit and integration
tests, doctests, proc macros, `cdylib` and `staticlib` crate types, and build
scripts compiled by any builder, including check, clippy, and documentation
builds.

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
assets. Topcoat `asset!` declarations are indexed and bundled as described in
[Topcoat assets](topcoat-assets.md).

## Compressed crate artifacts

Crate outputs are compressed archives staged per compilation mode; see
[Crate archives and dependency staging](crate-archives.md).

## Checks

`<crate>:check` type-checks a workspace crate with `rustc --emit=metadata`.
`<crate>:test` compiles and runs unit tests and integration tests with
`rustc --test` and test overrides. Integration test binaries compile in
parallel, bounded by the build cores, before any of them runs; they then run in
file order. `<crate>:clippy` runs `clippy-driver` with
warnings denied by default. `<crate>:doc` builds rustdoc output.
`<crate>:doctest` runs rustdoc tests for library crates.
`<crate>:unused-dependencies` compiles every target with rustc's
`unused_crate_dependencies` lint; see
[Unused dependency checks](unused-dependency-checks.md). All checks except
package checks share one dev-profile check graph; `<crate>:check`, clippy, and
documentation stage only dependency metadata from it, and package checks
consume full release crates. See [Check profile selection](check-profiles.md). Package checks reuse the release
crate and binary derivations and are generated only
when `checks.packages.enable` enables them. The exported package
derivations are the production build targets that the
[CLI build command](../../../CLI/BUILD/design/execution.md) materializes; an
optimized binary's package is its optimized derivation while its package check
stays on the release build. `workspace:lock` is a read-only
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
