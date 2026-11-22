# Workspace options

Every workspace setting controls the same strongly-typed option module. The
module values originate from the build tables of `.bloomery/config.toml` as
described in [Build configuration](configuration.md), except where a value must
be a Nix expression. Options are grouped by responsibility.

## Source and files

`root` is a constructor argument. `build.cargoToml` and `build.cargoLock`
default to `root`-relative paths, and `build.bloomeryLock` is auto-detected when
the lock file exists. `build.members` restricts the built members to the listed
crate names; omitting it builds every discovered member.

## Toolchain and linker

`build.toolchain` selects `rustc`, `clippy`, `cargo`, `lld`, `mold`, `stdenv`,
and the linker strategy. The default linker is `lld` on Linux and the system
linker elsewhere; `lld`, `mold`, or the system linker may be selected
explicitly.

## Compilation profiles

`build.profile.release`, `build.profile.dev`, and `build.profile.test` accept
`optLevel`, `lto`, `codegenUnits`, `panic`, `strip`, `debuginfo`, `targetCpu`,
`overflowChecks`, `linker`, and `linkArgs`. Values are type-checked and lowered
to `rustc` flags. Each profile resolves in layers, later layers winning field by
field: Bloomery defaults, `Cargo.toml` `[profile.*]`, then explicit options.
`build.profileName` selects the `Cargo.toml` profile behind the release profile.

| Profile | Bloomery defaults | Compiles |
| --- | --- | --- |
| release | `optLevel = 3` | packages, binaries, and the crates they link |
| dev | `optLevel = 0`, `lto = "off"`, `codegenUnits = 256`, `debuginfo = 2` | `<bin>:dev` apps and their workspace crates |
| test | the dev profile, then `debuginfo = "line-tables-only"` | the [check graph](../../GRAPH/design/check-profiles.md) |

`optLevel`, `codegenUnits`, `debuginfo`, and `overflowChecks` apply to every
crate a profile compiles. `lto`, `panic`, `strip`, `targetCpu`, `linker`, and
`linkArgs` apply to the final binary link.

## Package visibility

`build.libPackages` exposes `<crate>:lib` packages and `build.devPackages`
produces dev-profile apps.

## Flags

`build.flags` supplies extra flag lists applied per build phase. `rustc`
applies to every crate compilation in every profile; `test` adds to unit-test
harnesses, `clippy` to clippy runs, and `doc` and `doctest` to `rustdoc`. Flag
lists never carry codegen settings: a `rustc`, `test`, or `clippy` entry that
sets `opt-level`, `codegen-units`, `debuginfo`, or `overflow-checks` (including
`-O` and `-g`) is an evaluation error naming the profile field to use.

## Overrides

`overrides` maps crate names or package IDs to per-crate settings: native
build inputs, build inputs, runtime tool dependencies, rustc and rustdoc flags,
environment, explicit features, fileset or src, release and dev profile
fragments, assets, asset directories, additive test-only inputs, and additive
training inputs under the `optimize` namespace. Built-in
sys-crate overrides, colocated `overrides.nix` files, and the `overrides`
constructor argument merge in that order with explicit values winning. Override
keys match package IDs, hyphenated names, and underscored names.

## Optimization

`optimize` opts a discovered binary into an optimized build. Each entry is
keyed by binary name and carries `enable` and a repository-relative `script`
required while a training stage is enabled. The `pgo` and `bolt` sub-tables
select the stages independently and `pgo.scope` chooses between workspace-only
and whole-closure instrumentation. An optional `systems` table keyed by system
name gates the
optimized build to listed systems and is the only place to set `targetCpu`,
which also tunes the final binary when both stages are disabled. The
stages run in the fixed PGO then BOLT order and the result of the last enabled
stage replaces the binary's exported package and release app, while
package checks, when enabled, keep the release build. The per-crate
override `optimize` namespace carries `nativeBuildInputs`, `buildInputs`, `env`,
and an additive training fixture `fileset`; these Nix values stay in Nix rather
than `config.toml`. The pipeline is owned by
[Optimized binary generation](../../OPTIMIZE/design/optimization.md).

## Feature resolution

`build.unify` selects how resolved features lower into crate nodes. A value of
`true` (the default) unifies each package across the whole workspace cargo-style
into one derivation; `false` unifies each workspace member's closure separately,
so a shared package may compile once per member. `features.cratesIoIndex`
supplies a local crates.io index for the no-lock fallback.

## Development shell and checks

`devShell` controls shell generation, extra packages, and the shell hook.
`checks.enable` enables or disables generated Nix checks and defaults to off.
Each Nix check has its own table and `enable` setting: `checks.crates`,
`checks.packages`, `checks.workspaceDependencies`, `checks.noDefaultFeatures`,
`checks.vulnerabilities`, `checks.licenses`, and `checks.bans`. Each category
contributes its entries only when both its switch and the global switch are
enabled. Dependency policy checks evaluate each selected member's resolved
context. `checks.lockFreshness.enable`
defaults to off and controls the stale-lock policy independently. Enabled
package checks reuse the release crate and binary derivations, while
[build](../../../CLI/BUILD/README.md) realizes the exported production
packages. The workspace dependency and no default features manifest checks are
available when their category toggles are enabled. Vulnerability, license, and
package-ban policies are described in
[Dependency policy checks](../../GRAPH/design/dependency-policy-checks.md).

## Formatter ordering

`formatters` controls Bloomery's formatter set. Each known formatter, built in
or supplied through the constructor's `extraFormatters`, exposes an `enable`
toggle and `before`/`after` lists of other formatter names. Enabled formatters
form a directed graph whose edges are resolved into a total order for the
treefmt run; cycles, self-references, and names that are neither built in nor
supplied by the flake are evaluation errors.
