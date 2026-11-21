# Per-system outputs

The `mkFlake` constructor returns this workspace output set for each configured
system.

## Packages

`packages` contains one derivation per discovered binary named `<bin>`,
`<crate>:lib` for libraries when library packages are enabled, and `default`
pointing at the preferred binary. For a binary enabled in `[optimize]`, `<bin>`
is the optimized derivation. Dev-profile binaries are never packages.
`default` prefers a binary named `default` and otherwise uses the first
discovered binary. These are the production derivations the
[CLI build command](../../../CLI/BUILD/design/execution.md#catalog-evaluation)
materializes. Package checks, when enabled, continue to use the release build,
so no check realizes a training run.

## Apps

`apps` contains runnable release binaries named `<bin>`, dev-profile binaries
named `<bin>:dev` when dev packages are enabled, documentation servers named
`<crate>:doc`, and `default` for the preferred binary. For a binary enabled in
`[optimize]`, the `<bin>` app is the optimized derivation; dev apps remain
dev-profile builds. Each app is an attribute set with `type = "app"` and a
`program` path.

## Checks

For every evaluated system, `checks` includes
`"core:treefmt-check"` independently of Rust check and package-check settings.
It validates workspace files selected by the resolved formatter include globs
and includes the treefmt project config. When Git metadata is available, only
tracked files enter its source set; otherwise the Nix workspace root supplies
the source. It does not modify the workspace.

When checks are enabled, `checks` contains `<crate>:check`, `<crate>:test`,
`<crate>:clippy`, `<crate>:doc`, and `<crate>:doctest` for workspace crates;
`<crate>:doctest` exists only for crates with a library entrypoint.
`<crate>:check`, clippy, and documentation checks compile their dependency
crates metadata-only; test and doctest compile workspace crates with the dev
profile; package checks use full release crates. `<crate>:check` is generated
only when `checks.crates.enable` is enabled; it is off by default. Package checks
`<bin>:bin` and `<crate>:lib` are included only when
`checks.packages.enable` is enabled; it is also off by default. Enabling
package checks reuses the release crate and binary derivations.
`workspace:lock` validates lock consistency, `workspace:dependencies`
validates dependency inheritance, and `workspace:default-features` validates
disabled default features; each manifest check follows its own check table's
`enable` switch. When enabled, dependency policies add per-member
`<crate>:vulnerabilities`, `<crate>:licenses`, and `<crate>:bans` checks for the
member's resolved dependency context. Bloomery never generates a recursive
`bloomery:check` output.

## Derivation and metadata outputs

`playwrightTest` constructs explicit browser-test derivations for registration
through `extraOutputs`, as defined by [the runner](../../PLAYWRIGHT/design/runner.md).

`crates` maps package IDs to release crate derivations and `cratesDev` maps
workspace package IDs to dev-profile derivations. `lock` exposes the parsed
lock model and `config` exposes the evaluated options.

## Check catalog

`healthSources.<system>` projects each workspace crate's manifest path and
production Rust targets for CLI code health.

`bloomeryCatalog.<system>` and `bloomeryPlan.<system>` carry evaluated catalog
facts for `bloomery check`. Each is a derivation that is never built: its JSON
record lives in one environment attribute, so `nix derivation show` of the
flake attribute reads it and Nix's flake evaluation cache answers that read for
an unchanged locked workspace. They are computed from the final
`checks.<system>`, including `extraOutputs` checks, elevated sub-flake checks,
and generated Billet checks, and exist for every evaluated system.

The `catalog` attribute of `bloomeryCatalog.<system>` records every check
attribute name except an exact legacy `bloomery:check`, whether that legacy
attribute exists, the evaluated `core:treefmt-check` derivation path, the
`formatter.<system>` derivation path and executable, and the
`healthSources.<system>` projection. The `plan` attribute of
`bloomeryPlan.<system>` records every other check attribute except
`core:treefmt-check` with its derivation path, or an evaluation failure
marker when the attribute does not evaluate to a derivation. Recorded
derivation paths are plain store references: a record keeps each recorded
`.drv` file without depending on its outputs, so reading a record realizes
nothing.

A plan budget check evaluates `bloomeryPlan.<system>` for fixture workspaces in
a fresh evaluator with a private store and fails when the evaluator's thunk
count exceeds that fixture's budget for the system. Budgets sit about 10%
above the measured count for the pinned nixpkgs and are recorded only for
systems that run the check.

## Development shell

When enabled, `devShell` is a shell containing `rustc`, `clippy`, `cargo`,
`nix-fast-build`, and the Bloomery CLI, plus the selected linker and user
packages, with the configured shell hook. It also carries the Tailwind CLI and
`TAILWIND_CLI` when a workspace build script needs them, as described in
[Topcoat assets](../../GRAPH/design/topcoat-assets.md#tailwind). When disabled, `devShell` is null and
`mkFlake` omits the default shell.

## Formatter

`mkFlake` exposes `formatter.<system>` as a treefmt wrapper built from the
resolved formatter graph. The wrapper includes the default formatter set, with
`toml-sort` enabled, and runs the enabled formatters in the resolved order. The
output is present for every selected system regardless of check enablement.
