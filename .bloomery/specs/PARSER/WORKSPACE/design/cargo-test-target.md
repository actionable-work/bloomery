# Declared Cargo test entrypoints

`RustCfg.cargo_test` selects one integration-test target by canonical repository-
relative `manifest` path and nonempty `name`. The manifest filename is
`Cargo.toml`. Selection reads only the authorized tree. No Cargo command, host
probe, workspace discovery or feature unification runs.

`bloomery_workspace::cargo::load_test_target` projects a unique, explicitly
named `[[test]]` record with an explicit `path`. Paths resolve relative to the
manifest and normalize dot segments within the repository. Escaping paths,
missing entrypoint files, malformed selected fields, duplicate selected names,
missing declarations and inferred entrypoints produce located diagnostics.
This projection does not validate the entire Cargo manifest.

The report records manifest, target name, canonical entrypoint, sorted unique
`required-features`, harness mode and eligibility. All required features must
be present in the caller's effective feature set. An absent feature set is
unresolved when requirements exist; an empty requirement set is satisfied.
Custom harnesses remain unresolved. No default test flags are inferred.

Rust tests and bindings in the selected entrypoint receive its eligibility.
[Direct out-of-line children](../../SCANNING/design/rust-modules.md) additionally
inherit entrypoint and declaration cfg gates. Other Rust files remain unresolved.
Failed target projection leaves Rust target conditions unresolved while local
cfg discovery continues independently. Disabled Rust scanning leaves target
projection disabled. Rust coverage requires both satisfied local cfg and target
conditions. All literal bindings and orphan/manual findings remain visible;
other scanners remain syntactic. Relations stay partial and absent coverage
unresolved. No execution outcome or approval is implied.

Report/parser contracts are version 8; specification schema 1 is unchanged.
The normalized selector participates in scope identity. Manifest and entrypoint
bytes participate in tree identity; the resolved projection and conditions
participate in report identity. `completeness.rust_target` describes entrypoint
projection, independently of local cfg and whole-target/module completeness.
Automatic Cargo target discovery, other target kinds, transitive/inline module
membership, feature unification and macro expansion remain explicit limitations.
