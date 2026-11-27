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
switches, explicit target paths, `harness`, and `required-features`. Targets
outside the member's source selection are not compiled. The source selection
includes examples and benchmarks.

Each unit is compiled to metadata with the member's resolved features, or its
crate override's `features`, as `feature` cfgs, the build script's `rustc-cfg` output, the test profile flags,
and rustc's `unused_crate_dependencies` lint. Lints are capped at warnings, so
`-Dwarnings` and source-level `deny` attributes never fail the build; a
source-level `allow` of the lint marks its crate's externs as used. A unit that
fails to compile fails the check with the compiler's diagnostics.

| Unit | Externs | Satisfies |
| --- | --- | --- |
| library, binaries | normal dependencies | `dependencies` |
| library and binary unit tests, integration tests, examples, benchmarks | normal and dev-dependencies | `dev-dependencies` |
| build script | build dependencies | `build-dependencies` |

Binaries and test units also receive the member's own library. Harnessed tests
and benchmarks compile with `--test`; examples and `harness = false` targets
compile as binaries.

## Usage policy

The checked declarations are the member's selected lock edges for the target
platform, plus build edges for the host platform, named by their extern names.
A declaration is used when at least one unit that satisfies its section passes
its extern and the lint does not report it. A normal dependency used only by
test units is unused; it belongs in `dev-dependencies`.

`checks.unusedDependencies.exceptions` lists dependency package names that are
never reported. Excepted dependencies are still passed to the compiler.

## Diagnostics

A failing check prints `UNUSED DEPENDENCY CHECK FAILED for <member>`, then one
line per unused declaration naming its section, package, and extern name, and
a repair hint naming `checks.unusedDependencies.exceptions`.
