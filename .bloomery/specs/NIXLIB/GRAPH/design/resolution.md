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

A present `bloomery.lock` supplies the resolved global package graph and one
resolution context per workspace member. The global `packages` view records, for
every package, its active features and activated dependencies as resolved with
workspace-wide unification. Each context records the same information for one
member's fully-unified dependency closure. The lock also records proc-macro and
edition metadata per package and a SHA-256 digest of `Cargo.lock`. Contexts are
independent of `build.unify`, so changing that setting neither invalidates the
lock nor changes its bytes. `workspace:lock` reports a digest mismatch, and
`checks.throwOnOutOfDate` turns it into an evaluation error.

## Feature unification modes

Feature sets must be unified within every final artifact: Rust treats two builds
of the same crate with different features as distinct crates, so mixing their
items inside one artifact breaks type and trait identity. `build.unify`
therefore selects how much is unified, never whether a single artifact is
unified.

With `build.unify = true` (the default), the build uses the global `packages`
view: one cargo-like node per package, unified across the whole workspace.

With `build.unify = false`, each workspace member resolves against its own
context. Every binary, library, test, and documentation artifact of that member
is built from that member's context, so an artifact never mixes crate instances
from two contexts. A package shared by several members can therefore compile
once per member closure.

## Index fallback

Without `bloomery.lock`, a configured `features.cratesIoIndex` drives the pure
Nix solver: local manifests describe workspace crates and index entries describe
registry crates. The solver resolves the global view and one unified context per
workspace member to a fixed point, honoring optional dependency activation and
`dep:`/`dep?/feat` syntax, and yields the same features and activated edges as
the lock path. `build.unify` selects lowering only. Without either input,
evaluation fails with guidance to run `bloomery sync`.

## Crate nodes and edges

Every package in `Cargo.lock` becomes a release crate node; `cratesDev`
re-evaluates workspace members with dev flags when dev packages are enabled
while sharing non-workspace nodes. The global graph exposes one node per package
keyed by `<name>-<version>`. Each resolution context exposes one node per package
in that context, keyed by the context and the package ID, so the same package may
have distinct derivations in different contexts.

A node's derivation inputs are the crate nodes of its activated dependencies.
Edges always follow the resolved graph for the node's context; a node never
depends on a package that its active features do not activate. Transitive crates
are exposed through a propagated closure so downstream compilation can resolve
indirect crates and proc-macro helpers.

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
resolved context or global view, then index-resolved features, and finally the
default feature set.

## Crate identity

Crate identity includes the package ID, active features, selected source
contents, toolchain, native inputs, flags, and profile. Distinct feature sets do
not collide. External crate derivations do not depend on unrelated local
content.

## Evaluation boundary

Graph resolution reads manifests, `Cargo.lock`, and `bloomery.lock` only. It
never runs Cargo, imports from a derivation, or fetches sources during
evaluation; derivation building and source fetching happen only when Nix
realizes the graph.
