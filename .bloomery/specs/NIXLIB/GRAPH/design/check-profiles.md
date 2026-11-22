# Check profile selection

Checks need neither optimized object code nor separate crate builds per check
kind. They compile against a single check graph built with the test profile,
while package checks and exported binaries keep the release profile.

## Test profile

The test profile inherits the dev profile, except that `debuginfo` defaults to
`line-tables-only`. Line tables keep panic backtraces with file and line, and
leave debug assertions and overflow checks unchanged; only variable inspection
in a debugger is lost. `build.profile.test` and `Cargo.toml` `[profile.test]`
override individual fields.

## Check graph modes

`checks.profile` selects which crates of the check graph compile with the test
profile:

| Mode | Test-profile crates | Other crates |
| --- | --- | --- |
| `dev` (default) | workspace crates | release crates |
| `cargo` | every crate | none |
| `release` | none | release crates |

`dev` rebuilds only what a workspace edit invalidates, sharing registry crates
with release builds. `cargo` matches `cargo test`: registry crates also keep
debug assertions and overflow checks, at the cost of a second registry crate
build. `release` shares the whole graph with package checks and binaries;
library code outside a crate's own unit tests loses debug assertions and
overflow checks. Build-script dependencies come from the release host graph in
every mode.

## Linking check crates

The `<crate>:test` and `<crate>:doctest` checks resolve the crate under test and
its dependency closure through the check graph. The unit-test harness of the
crate under test compiles with the test profile in every mode.

## Availability

The check graph is available whenever checks are enabled, whether or not
`build.devPackages` exposes dev-profile apps. `cratesDev` and dev-profile apps
remain gated by `build.devPackages`, so check profile selection is internal to
the check derivations.

## Type-checking checks

`<crate>:check` type-checks a workspace crate with `rustc --emit=metadata`. The
`<crate>:check`, clippy, and documentation checks resolve their dependency
closure through the same check graph as test and doctest, and stage only each
dependency's `meta.tar.zst`. A dependency crate is therefore built once and
shared by every check of a workspace crate; no check builds a separate
metadata-only crate variant. The checked crate itself compiles with the test
profile, so `cfg(debug_assertions)` matches the test harness.

## Release consumers

Package checks and exported binaries keep the release profile. A single check
artifact never mixes crate instances from different profiles.

## Builder tests

The builder test suite covers check-graph consumption directly:

- a test check whose workspace dependency resolves to a test-profile variant;
- a doctest check whose workspace dependency resolves to a test-profile
  variant; and
- check, clippy, and documentation checks whose workspace dependency resolves to
  the same test-profile variant as the test check.
