# Lock data and graph resolution

## Workspace discovery

Workspace members come from explicit `build.members` or from the workspace
`Cargo.toml`: the root package, `[workspace.dependencies]` entries with a
`path`, and `[workspace] members` entries including `*` globs, minus
`[workspace] exclude` entries. A candidate without a manifest containing a
package name is not discovered.

## Cargo.lock model

`Cargo.lock` is parsed with `builtins.fromTOML`. Each package becomes a node
identified by `<name>-<version>` carrying its source kind, checksum, and
resolved dependency IDs. Missing or ambiguous dependency references are
evaluation errors that name the offending dependency.

## bloomery.lock

When `bloomery.lock` is present, it supplies each package's active features and
active dependency edges plus proc-macro and edition metadata. The lock records
a SHA-256 digest of `Cargo.lock`. `workspace:lock` reports a mismatch, and
`checks.throwOnOutOfDate` turns it into an evaluation error.

## Index fallback

Without `bloomery.lock`, a configured `features.cratesIoIndex` drives the pure
Nix feature solver: local manifests describe workspace crates, index entries
describe registry crates, and active features, optional dependency activation,
`dep:`/`dep?/feat` syntax, and workspace unification resolve to a fixed point.
Without either input, evaluation fails with guidance to run `bloomery sync`.

## Crate nodes and edges

Every package in `Cargo.lock` becomes a release crate node keyed by
`<name>-<version>`; `cratesDev` re-evaluates workspace members with dev flags
when dev packages are enabled while sharing non-workspace nodes. A node's
derivation inputs are the crate nodes of its active resolved dependencies,
which forms the DAG. Graph edges follow the lock's active dependency IDs when
available and all resolved `Cargo.lock` dependencies otherwise. Transitive
crates are exposed through a propagated closure so downstream compilation can
resolve indirect crates and proc-macro helpers.

## Sources

Workspace nodes use filtered member sources as specified by
[NIX/SOURCES](../../../NIX/SOURCES/README.md). Registry nodes are fetched from
`static.crates.io` with the checksum recorded in `Cargo.lock`. Git nodes are
fetched at the pinned revision from the package source. Any other source is an
evaluation error.

## Override and feature precedence

Effective overrides merge built-in sys-crate defaults, colocated
`overrides.nix` files, and explicit options, with explicit values winning.
Override keys accept package IDs, hyphenated names, and underscored names. A
package's active features come from a per-crate override when present, then the
lock manifest, then index-resolved features, and finally the default feature
set.

## Crate identity

Crate identity includes the package ID, active features, selected source
contents, toolchain, native inputs, flags, and profile. Distinct feature sets
do not collide. External crate derivations do not depend on unrelated local
content.

## Evaluation boundary

Graph resolution reads manifests, `Cargo.lock`, and `bloomery.lock` only. It
never runs Cargo, imports from a derivation, or fetches sources during
evaluation; derivation building and source fetching happen only when Nix
realizes the graph.