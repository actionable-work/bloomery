# Derivation identity and rebuild boundaries

## Content identity

A filtered tree's store identity depends on its selected contents, relative
layout, and a stable source name, not the absolute checkout location or enclosing
flake snapshot hash. Identical selected trees from different flake snapshots
produce identical source store paths.

Every build-time path reference uses an isolated tree or an explicit external
input. Raw workspace paths do not leak into derivation attributes, generated
shell scripts, environment variables, or symlink targets. Removing string
context is not a substitute for materializing required inputs in the store.
Evaluation may read manifests and locks from the flake snapshot; generated
Rust derivations receive the resulting relevant metadata, not the snapshot path
as an incidental build input.

Derivation identity includes selected sources, toolchain, native inputs,
compiler flags, profiles, resolved features, and active dependency derivations.
Unchanged values produce unchanged derivation paths. Registry and pinned Git
crate derivations do not depend on unrelated local workspace content.

## Invalidation boundaries

| Changed input | Required invalidation boundary |
| --- | --- |
| Unselected README, standalone prose, or `.bloomery/specs` content | No generated Rust package or Rust check changes identity. |
| Selected Rust source or embedded Markdown | Owning member's consuming derivations and active transitive dependents; unrelated members remain stable. |
| Test-only fixture or test-support file | Consuming tests; production packages remain stable. |
| Bundled asset | Derivations consuming or installing that asset and their dependents; unrelated crate compilation remains stable. |
| Manifest, lock metadata, profile, feature, or toolchain | Derivations whose effective build inputs change. |

A fileset is conservative within a selected directory. This contract does not
require Rust reachability analysis or output-content equivalence analysis.
Dependency invalidation follows the active resolved graph, separately for dev
and release profiles.

## Automated verification

Use paired workspace snapshots with identical relevant contents and different
unselected contents. Compare selected source store paths and generated
`.drvPath` values under the same system, toolchain, and configuration. Cover
root packages, nested members, symlinked entrypoints, custom filesets, explicit
local paths, auxiliary inputs, workspace assets, and the repository
cli-app/sync support overrides across `mkWorkspace`, `mkFlake`, and flake-parts.

Negative controls change README and specification content without changing any
Rust derivation identity. Positive controls change Rust source, embedded
Markdown, test fixtures, support inputs, and assets; assert both the expected
changed identities and unaffected identities. Repository support controls pair
enclosing snapshots so unrelated prose preserves support and Rust consumer
identities, while selected support-file edits change consuming test identities
and leave release and dev production packages unchanged. A forked dependency
graph makes transitive invalidation and sibling stability observable.

Realize representative fixtures to verify embedded content, test fixtures,
symlink targets, and installed assets survive filtering. With baseline outputs
already realized, building an unchanged derivation reuses Nix's store result;
wall-clock timing and build-log counts are not the primary regression oracle.
