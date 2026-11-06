# Caller-supplied Rust cfg facts

`RustCfg` supplies optional effective features, explicit Boolean flags, named
sets of cfg values and an optional target label. No facts are inferred from the
host, target label, Cargo, build scripts or environment. An absent feature set
is unknown; an explicit empty set disables all features. Unknown flags and value
keys remain unknown; explicit false flags and empty value sets are known false.

`scan_tree_with_conditions` reuses shared source ingestion and the Rust discovery
visitor to expose owned local predicate expressions. The report consumer's
`RustCfg` evaluates file, inline-module and test predicates with three-valued
`all`, `any` and `not` logic. False dominates `all`;
true dominates `any`. Empty `all` is true and empty `any` false. Pure nested
`cfg_attr(..., cfg(...))` gates are evaluated. Bloomery metadata carriers remain
statically discoverable regardless of their disabled `cfg_attr` condition.

Each Rust test condition result identifies the test and reference locations,
its `satisfied`, `rejected` or `unresolved` state, and effective unresolved
predicate names. Unsupported or malformed predicates remain unresolved.
Conditional test attributes are retained as unresolved candidate sites when
cfg analysis is requested. Normal syntactic discovery is unchanged.

The report separately projects an optional
[declared Cargo test entrypoint](../../WORKSPACE/design/cargo-test-target.md);
its target condition does not change local cfg semantics.

These results describe local predicates only. Cargo feature unification,
required-features targets, out-of-line module ancestry and macro expansion
remain unsupported. A satisfied local predicate is not proof that a test is
compiled, executable or passing. No compiler, Cargo or subprocess runs.
