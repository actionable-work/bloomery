# Sub-flake composition

`mkFlake` can compose independently evaluated sub-flakes into the main flake.
Composition is off by default and is enabled per sub-flake through
`.bloomery/config.toml`. Only a sub-flake's `packages`, `apps`, and `checks`
outputs can be elevated; every other output family stays with the sub-flake.

## Configuration

The `[flakes]` table is a parameterized family keyed by sub-flake name. Each
`[flakes.<name>]` table carries:

| Key | Type | Default within a present entry |
| --- | --- | --- |
| `path` | repository-relative path | required |
| `packages` | boolean | `false` |
| `apps` | boolean | `false` |
| `checks` | `"none"`, `"individual"`, or `"aggregate"` | `"none"` |

```toml
[flakes.tests-basic]
path = "tests/basic-workspace"
packages = true
apps = true
checks = "individual"

[flakes.tests-axum]
path = "tests/axum-workspace"
checks = "aggregate"
```

An absent `[flakes]` table, or a sub-flake table whose elevation keys are all
omitted, elevates nothing. A sub-flake table whose `path` is absent or empty is
an evaluation error, as is a `path` that escapes the repository root or names a
directory without a `flake.nix`.

## Evaluation

For each selected system, Bloomery imports `<root>/<path>/flake.nix`, calls its
`outputs` with the main flake's inputs, and reads that system's outputs. Each
sub-flake's `outputs` is evaluated once and read for every selected system.
Evaluation is pure: it uses no
`builtins.getFlake`, no network access, no impure mode, and no import-from-
derivation. A sub-flake that exposes no `packages`, `apps`, or `checks`
attribute for a selected system contributes nothing to that system.

## Nesting

A configured sub-flake may itself configure sub-flakes. Composition is
recursive: a sub-flake that evaluates `mkFlake` composes its own configured
sub-flakes into its outputs, and the parent elevates those outputs under the
parent's `<name>:` prefix.

The constructor injects the main flake's inputs, including the Bloomery
`mkFlake` binding, into every configured sub-flake. A nested `bloomery.mkFlake`
call therefore inherits those inputs without the sub-flake looping `self` back
into the construction. Arbitrary inputs not supplied by the constructor still
come from the main flake's `self`.

## Elevated packages and apps

When `[flakes.<name>].packages` is enabled, each named sub-flake
`packages.<system>` attribute is elevated as
`packages.<system>.<name>:<package>`. When `[flakes.<name>].apps` is enabled,
each named sub-flake `apps.<system>` attribute is elevated as
`apps.<system>.<name>:<app>`. The sub-flake's `default` attribute is elevated
under the same prefix, so the main workspace's `packages.default` and
`apps.default` are never replaced by a sub-flake. Dev-profile derivations and
all other output families stay out of the main flake.

## Elevated checks

When `[flakes.<name>].checks` is `"individual"`, each sub-flake
`checks.<system>` attribute is elevated as `checks.<system>.<name>:<check>`.
When it is `"aggregate"`, the sub-flake contributes one check at
`checks.<system>.<name>:checks` that depends on every sub-flake check for that
system; realizing the aggregate realizes them all, and a failing sub-flake
check fails the aggregate. When it is `"none"`, no sub-flake check is elevated.
Elevated checks are governed only by the per-flake `checks` mode; the main
workspace's `[checks]` toggles gate only its own generated checks.

## Output restrictions

Only `packages`, `apps`, and `checks` are elevated. `devShells`, `formatter`,
`legacyPackages`, `overlays`, `nixosModules`, `lib`, and every other output
family are never elevated, and no configuration key can request their
elevation.

## Name collisions

Every elevated attribute is namespaced by its sub-flake name. An elevated name
that duplicates another attribute in the same main-flake output and system is
an evaluation error. The main workspace's own outputs are generated unchanged
alongside the elevated outputs.
