# Unused dependency checks

## Generation

When `checks.enable` and `checks.unusedDependencies.compiler` are true, every
selected workspace member gets a `<crate>:unused-dependencies` check. The check
is read-only: it changes no manifests, lockfiles, or sources.

## Compilation units

The check enumerates the member's Cargo targets from its manifest and layout:
the library or proc-macro, binaries, integration tests, examples, benchmarks,
and the build script. It follows Cargo's inference from `src/lib.rs`,
`src/main.rs`, `src/bin`, `tests`, `examples`, and `benches`, the `auto*`
switches, explicit target paths, `harness`, `required-features`, and example
`crate-type`. The source selection includes examples and benchmarks.

A target outside the member's source selection, or whose `required-features`
the resolved features miss, is skipped. A `dep/feature` entry requires the
`dep` feature. No declaration of a section that a skipped target could
satisfy is reported: a skipped build script leaves `build-dependencies`
unchecked, a skipped library or binary leaves `dependencies` and
`dev-dependencies` unchecked, and any other skipped target leaves
`dev-dependencies` unchecked. The log and the check output name each skipped
target and the sections it leaves unchecked.

The resolved features follow the crate build: a crate override's `features`,
then the member's lock context, then `bloomery.lock`, then the manifest
resolution, then `default`. Without `bloomery.lock`, an edge that names no
extern uses the dependency's library crate name.

The build script runs with the environment Cargo gives it, including
`CARGO_CFG_*` for the target, `CARGO_FEATURE_*` for the resolved features,
`CARGO_PKG_NAME`, `CARGO_PKG_VERSION`, `CARGO_MANIFEST_LINKS`, `OUT_DIR`,
`TARGET`, `HOST`, and `RUSTC`. A build script that fails fails the check with
its captured output.

Each unit is compiled to metadata with the resolved features as `feature`
cfgs, the build script's `rustc-cfg` output, the test profile flags, and
rustc's `unused_crate_dependencies` lint forced to warn. Source-level
`allow` attributes cannot silence the lint, and lints are capped at warnings,
so `-Dwarnings` and source-level `deny` attributes never fail the build.
`checks.unusedDependencies.exceptions` is the only opt-out. A unit that
fails to compile fails the check with the compiler's diagnostics.

| Unit | Externs | Satisfies |
| --- | --- | --- |
| library, binaries | normal dependencies | `dependencies` |
| library and binary unit tests, integration tests, examples, benchmarks | normal and dev-dependencies | `dev-dependencies` |
| build script | build dependencies | `build-dependencies` |

Binaries and test units also receive the member's own library. Harnessed tests
and benchmarks compile with `--test`. Examples whose `crate-type` has no `bin`
compile as libraries; other examples and `harness = false` targets compile as
binaries.

## Usage policy

The checked declarations are the member's selected lock edges for the target
platform, plus build edges for the host platform, named by their extern names.
A declaration is used when at least one unit that satisfies its section passes
its extern and the lint does not report it. When the library's doctests run,
a dev-dependency is also used when a documentation comment or Markdown file
of the member starts a path with its extern name, or names it after `use` or
`extern crate`. A normal dependency used only by
test units is unused; it belongs in `dev-dependencies`. Only the active
feature set is compiled, so a non-optional dependency used only behind an
inactive feature is unused; it belongs behind that feature as an optional
dependency.

Only the direct declarations of workspace members are checked. A dependency
that no Rust source names, such as a `-sys` crate declared to select its
linkage or features, is unused; a `use name as _;` item in a target that
needs it, or an exception, records the intent.

`checks.unusedDependencies.exceptions` lists dependency package names that are
never reported. Excepted dependencies are still passed to the compiler.

## Diagnostics

A failing check prints `UNUSED DEPENDENCY CHECK FAILED for <member>`, then one
line per unused declaration naming its section, package, and extern name, and
a repair hint naming the `use name as _;` item for dependencies needed only for
linking and `checks.unusedDependencies.exceptions`.
