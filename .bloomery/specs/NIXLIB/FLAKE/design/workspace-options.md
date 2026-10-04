# Workspace options

Every integration evaluates the same strongly-typed option module. Options are
grouped by responsibility.

## Source and files

`root` is required. `source.cargoToml` and `source.cargoLock` default to
`root`-relative paths, and `source.bloomeryLock` is auto-detected when the lock
file exists. `source.members` restricts the built members to the listed crate
names; null builds every discovered member.

## Toolchain and linker

`toolchain` selects `rustc`, `clippy`, `cargo`, `lld`, `mold`, `stdenv`, and
the linker strategy. The default linker is `lld` on Linux and the system linker
elsewhere; `lld`, `mold`, or the system linker may be selected explicitly.

## Compilation profiles

`profile` and `profileDev` accept `optLevel`, `lto`, `codegenUnits`, `panic`,
`strip`, `debuginfo`, `targetCpu`, `overflowChecks`, `linker`, and `linkArgs`.
Values are type-checked and lowered to `rustc` flags. `Cargo.toml`
`[profile.release]` and `[profile.dev]` values provide the base; explicit
options override them. `profileName` selects the active package profile.

## Package visibility

`createLibPackages` exposes `<crate>:lib` packages and `createDevPackages`
produces dev-profile apps. The aliases `libPackages` and `packages.createLib`
override `createLibPackages`; `devPackages` and `packages.createDev` override
`createDevPackages`. Aliases win over the top-level toggles.

## Flags

`flags` supplies extra `rustc`, `test`, `clippy`, `doc`, and `doctest` flag
lists applied per build phase.

## Overrides

`overrides` maps crate names or package IDs to per-crate settings: native
build inputs, build inputs, rustc and rustdoc flags, environment, explicit
features, fileset or src, release and dev profile fragments, assets, asset
directories, and additive test-only inputs. Built-in sys-crate overrides,
colocated `overrides.nix` files, and explicit options merge in that order with
explicit values winning. Override keys match package IDs, hyphenated names, and
underscored names.

## Development shell, checks, and features

`devShell` controls shell generation, extra packages, and the shell hook.
`checks` enables or disables generated checks, includes or omits package build
checks, and optionally fails evaluation on an out-of-date lock.
`features.unify` selects workspace feature unification for index-based
resolution; `features.cratesIoIndex` supplies a local crates.io index.