# Per-system outputs

The `mkFlake` constructor returns this workspace output set for each configured
system.

## Packages

`packages` contains one derivation per discovered binary named `<bin>`,
`<crate>:lib` for libraries when library packages are enabled, and `default`
pointing at the preferred binary. Dev-profile binaries are never packages.
`default` prefers a binary named `default` and otherwise uses the first
discovered binary.

## Apps

`apps` contains runnable release binaries named `<bin>`, dev-profile binaries
named `<bin>:dev` when dev packages are enabled, documentation servers named
`<crate>:doc`, and `default` for the preferred binary. Each app is an attribute
set with `type = "app"` and a `program` path.

## Checks

When checks are enabled, `checks` contains `<crate>:test`, `<crate>:clippy`,
`<crate>:doc`, and `<crate>:doctest` for workspace crates; `<crate>:doctest`
exists only for crates with a library entrypoint. Package checks `<bin>:bin`
and `<crate>:lib` are included when package checks are enabled.
`workspace:lock` validates lock consistency, `workspace:dependencies`
validates dependency inheritance, and `workspace:default-features` validates
disabled default features; each manifest check is omitted when its `[checks]`
toggle is disabled. Bloomery never generates a recursive `bloomery:check`
output.

## Derivation and metadata outputs

`crates` maps package IDs to release crate derivations and `cratesDev` maps
workspace package IDs to dev-profile derivations. `lock` exposes the parsed
lock model and `config` exposes the evaluated options.

## Development shell

When enabled, `devShell` is a shell containing `rustc`, `clippy`, `cargo`,
`nix-fast-build`, and the Bloomery CLI, plus the selected linker and user
packages, with the configured shell hook. When disabled, `devShell` is null and
`mkFlake` omits the default shell.

## Formatter

`mkFlake` exposes `formatter.<system>` as a treefmt wrapper built from the
resolved formatter graph. The wrapper includes the default formatter set, with
`toml-sort` enabled, and runs the enabled formatters in the resolved order. The
output is present for every selected system regardless of check enablement.