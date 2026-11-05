# Library names and dependency aliases

## Identities

Package identity, library target name, and dependency-edge name are independent.
`different-name 0.1.0` can publish `actual_library`; an unrenamed consumer imports
`actual_library`, while `renamed = { package = "different-name", ... }` exposes
`renamed`. Both consumers link the same library artifact. `md-5 0.11.0` publishes
`md5`. No package-specific naming rules apply.

The package ID selects source, version, features, overrides, graph nodes, and
derivation identity. Cargo's library target name selects `--crate-name`,
`CARGO_CRATE_NAME`, library filenames, archive member names, and propagated
artifact metadata. `CARGO_PKG_NAME` retains the package name. Each binary, test,
and build-script compilation retains its own target name.

## Lock contract

The version-3 lock records optional package `lib-name` and resolved
`dependency-edges` in global and per-member records. The
[sync contract](../../../CLI/SYNC/design/sync.md#library-names-and-dependency-edges)
defines their fields and canonical order. A missing `lib-name` denotes a package
without a library; it does not request a package-name fallback. Proc macros use
their Cargo library target name and retain their host-executable artifact type.

Unsupported lock versions fail evaluation with `bloomery sync` guidance.
Missing or invalid naming metadata fails before scheduling a dependent build;
an unchanged Cargo.lock digest does not make an incomplete lock valid. Lock
validation is read-only. Sync can regenerate the supported contract without
evaluating the consumer's build graph.

## Graph lowering

Lower each selected edge to its exact dependency node and extern identifier.
Select nodes by resolved package identity, including version and resolution
context. Keep multiple aliases pointing at one node; keep separate nodes when
aliases select different versions. Deduplicating derivation inputs must not
deduplicate distinct extern bindings.

Normal edges serve target compilations, build edges serve host build-script
compilations, and dev edges serve the applicable test units. Evaluate target
conditions for the applicable Cargo build unit; preserve proc-macro host
execution and host/target dependency selection. Mutually exclusive edges do not
produce duplicate or incorrect extern bindings. Optional or otherwise inactive
edges produce none.

Global, per-member, release, dev, metadata-only, and optimized graphs use the
same naming contract. An alias in one member's context cannot change another
context's bindings or the dependency artifact's intrinsic name. Feature
requests use declaration keys independently of the resolved extern name.

The index fallback retains the same naming semantics and evaluation boundary.
Local manifests supply workspace library names; external library names are
read from fetched manifests during realization and published in artifact
metadata. Resolved index edges retain declaration aliases and exact package
selection; an unrenamed edge uses its dependency's published library name.
Evaluation never fetches source or imports a built manifest to learn a name.

## Builder wiring

Every builder receives structured direct dependency bindings rather than
searching manifest text for aliases. Library, binary, metadata, unit-test,
integration-test, doctest, rustdoc, Clippy, and build-script invocations use
`--extern <edge-name>=<artifact-path>`. Renaming does not rename the artifact or
add an implicit binding under the package-derived name.

Artifact metadata records the intrinsic library name separately from package
identity. Archive extraction and primary-library lookup use that intrinsic
name, including `.rmeta`, `.rlib`, and proc-macro dynamic libraries. Native link
metadata and transitive artifact closures retain their existing contracts.
An externally supplied uncompressed dependency retains its supported layout;
the consumer's binding still uses the selected edge name.

Binaries, integration tests, and doctests link their package's own library
under its library target name. A package's own library binding and third-party
dependency aliases follow Cargo's namespace rules. Transitive artifacts remain
available for linking without acquiring additional direct extern bindings.

## Automated verification

Use a synthetic package `different-name` with `[lib] name = "actual_library"`,
an unrenamed consumer, and an explicitly renamed consumer. Include a workspace
member whose own library has a custom name, with binary, unit, integration, and
documentation sources that import the expected identifiers. Each builder's
fixture must use the dependency so a stub or an unused extern cannot pass.
Assert intrinsic names in compiled outputs and propagated metadata as well as
successful compilation. Exercise ordinary and compressed dependency layouts.

Generate metadata through normal sync. Verify aliases declared inline, in
dependency tables, through workspace inheritance, and under target conditions.
Cover multiple versions of one package, multiple aliases for one package where
Cargo permits them, optional activation, proc macros, build dependencies, and
both unification settings. Repeat sync to assert byte-identical output and
check unsupported and incomplete locks fail with repair guidance.

A public fixture depends on `object_store = "=0.14.2"` with
`default-features = false` and `features = ["aws"]`. Its library and binary
exercise object_store AWS APIs without credentials or network operations; its
unit, integration, and documentation tests use those APIs as well. Normal sync
produces its locks. Require its library and binary package builds, metadata
check, unit and integration tests, doctests, documentation, and Clippy to pass
through `bloomery check`, with the same checks included in full flake validation.

Retain the public compatibility fixtures for direct workspace construction,
`mkFlake`, flake-parts, package/app/check exports, and the index fallback.
Regenerate repository fixture locks through normal sync when required by the
supported lock contract; never patch vendored sources or hand-edit generated
metadata to make a naming fixture pass.
