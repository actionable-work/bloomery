# Manifest policy checks

Bloomery realizes two read-only workspace manifest policy checks alongside the
graph-derived checks. Both read the root `Cargo.toml` and every workspace member
`Cargo.toml` during evaluation, and both are gated by `checks.enable` plus their
own `[checks]` toggle. A violation fails the derivation and reports the
offending member and dependency with the repair.

## Name normalization

Dependency tables and their keys are normalized to Cargo's canonical
hyphenated names at evaluation time before the policies are applied, so
`default_features`, `build_dependencies`, and `dev_dependencies` are equivalent
to `default-features`, `build-dependencies`, and `dev-dependencies`.

## Workspace dependency policy

The `workspace:dependencies` check requires every dependency entry in a
workspace member's `[dependencies]`, `[dev-dependencies]`,
`[build-dependencies]`, and target-specific dependency tables to inherit from
`[workspace.dependencies]`. An entry satisfies the policy when it sets
`workspace = true`; a bare version string or an entry declaring its own `path`,
`git`, `version`, or `registry` source is a violation. The report names
`[workspace.dependencies]` and `workspace = true` as the repair.

## No default features policy

The `workspace:default-features` check requires every dependency consumed by a
workspace member to disable default features. The effective value is the member
entry's `default-features` when present, otherwise the referenced
`[workspace.dependencies]` entry's value, otherwise Cargo's default of enabled.
Every `[workspace.dependencies]` entry sets `default-features = false`. The
report names `default-features = false` as the repair.
