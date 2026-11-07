# Static report

`bloomery_check::report::analyze_tree(tree, config)` returns a versioned,
serializable report. `analyze(root, config)` uses the shared filesystem adapter.
Both use the repository loader, scanners and relational validator. Repository
files are the only specification discovery source.

The report snapshots the validated tree before analysis. The snapshot includes
canonical file paths, raw bytes and inferred or explicit directories, excluding
the shared tree adapter's ignored paths and filesystem symlinks. Binary content
participates in identity; only documents selected for parsing require UTF-8.
Unavailable advertised content or path collisions produce explicit failed tree
analysis, without a tree identity or downstream findings.

## Identity

Report format version 5 binds SHA-256 identities for the snapshot, the complete
typed configuration with defaults, the active specification subtree, and the
report payload. Hash inputs are compact canonical JSON with sorted object keys;
file bytes are represented by their SHA-256 digest. Directory identity includes
empty directories. Ordering and filesystem root do not affect identity.
Specification identity includes active subtree files and directories only when
specification loading succeeds; disabled specifications have an empty set.
The payload names the parser contract and specification schema versions.
The report identity omits its own `identity` field from the hashed payload.
Parser contract versions change when discovery semantics change; schema versions
change when specification interpretation changes. These identify contracts,
not a compiled executable or an execution attestation.

## Facts and completeness

The report contains requirement definitions, discovered test sites and literal
references, binding resolution, stable diagnostic codes, notes and repository-
relative locations. Locations preserve available one-based Unicode scalar start
and exclusive end line/column positions under the shared
[source span contract](../../SCANNING/design/source-spans.md). Unknown positions
remain null. The parser contract is `bloomery-static-syntax-v5`; the specification
schema remains `bloomery-grouped-specifications-v1`.
Orphan and manual-automation findings retain the full reference location.

Each tree, specification, discovery and relational stage has explicit state.
Scanner enablement and no-matching-files states remain visible. An aggregate
scanner failure marks enabled scanner facts unavailable. Failed stages
leave dependent stages unattempted. A successful discovery stage can be partial
when scanners are disabled or match no files. Missing bindings are unresolved
when discovery is partial; only complete discovery produces missing-test findings.
Unknown IDs and references to manual requirements retain relational findings.
Specification loading failure or disablement prevents relational validation;
independent source discovery remains available.

Scope defaults to syntactic over the configured source patterns.
Static completeness describes completion of these stages, independent of
whether diagnostics exist. Reports contain no test outcomes or hosted approval.

## Explicit Rust cfg scope

`analyze_tree_with_cfg(tree, config, cfg)` and `analyze_with_cfg(root, config,
cfg)` attach [local Rust predicate results](../../SCANNING/design/rust-cfg.md)
to tests and bindings. Scope includes the normalized caller context, its own
input identity, resolution descriptions and explicit limitations. The parser
contract is `bloomery-static-syntax-v5`; specification schema 1 is unchanged.
Default calls retain syntactic behavior and null condition annotations.

Cfg completeness describes only the requested local predicate analysis. Disabled,
unmatched, failed and unresolved analysis remains explicit. Cargo feature
resolution, module ancestry and macro expansion remain unsupported. Relations
are partial whenever cfg scope is requested. Only satisfied Rust conditions
contribute bindings to coverage; rejected and unresolved references remain in
the report. Absent bindings remain unresolved. Static orphan/manual findings
continue to validate literal IDs and retain their locations regardless of cfg.

An optional `cargo_test` selector in the cfg context attaches a
[declared integration-test entrypoint](cargo-test-target.md) projection and
per-test/binding target conditions. Local cfg and target projection have
independent completeness states. Rust coverage requires both conditions when
target selection is requested. Direct module projection has independent
completeness, declaration spans and inherited target conditions; other Rust
files remain unresolved.
