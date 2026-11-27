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
`crate-type`. Targets
outside the member's source selection are not compiled. The source selection
includes examples and benchmarks.

Each unit is compiled to metadata with the member's resolved features, or its
crate override's `features`, as `feature` cfgs, the build script's `rustc-cfg` output, the test profile flags,
and rustc's `unused_crate_dependencies` lint forced to warn. Source-level
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
its extern and the lint does not report it. A normal dependency used only by
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
