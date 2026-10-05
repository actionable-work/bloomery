# Build configuration

Every build setting that `mkFlake` evaluates comes from
`.bloomery/config.toml`. Settings that require Nix values stay in the
constructor or in `overrides.nix`.

## Configuration source

Build settings live in top-level tables: `[build]`, `[toolchain]`,
`[profile.release]`, `[profile.dev]`, `[flags]`, `[devShell]`, `[checks]`,
`[features]`, and `[formatters]`. The `[specs]` and `[scanners.*]` tables
belong to the CLI; the flake interface ignores them.

The CLI catalogs and type-validates the build keys before evaluation, using the
[schema catalog](../../../PARSER/CONFIGURATION/design/schema-catalog.md). The
flake interface remains authoritative for build-table semantics,
evaluation-time package resolution, and any validation that requires Nix
values.

## Required configuration

`.bloomery/config.toml` is required. All repository commands and `mkFlake` fail
when it is absent; the exemptions are parser help, the `init` command that
bootstraps it, and the `config` command whose mutating operations create it
with a warning. When
the file is missing, `mkFlake` fails evaluation with a `bloomery:`-prefixed
message naming the missing path.

## Value encoding

TOML values map to the workspace option module as follows:

- Booleans, integers, strings, lists of primitives, and inline tables map to
  the matching option types.
- A package-valued option is a string naming a nixpkgs attribute path, such as
  `"rustc"` or `"llvmPackages.lld"`. It is resolved against the selected
  system's `pkgs`.
- A path-valued option is a repository-root-relative string.

## Build table

`[build]` carries `cargoToml`, `cargoLock`, `bloomeryLock`, `members`,
`profileName`, `libPackages`, `devPackages`, and `unify`. Omitted source path
keys default below the root; an omitted `bloomeryLock` key auto-detects the lock
file. The `libPackages` and `devPackages` booleans are the visibility toggles,
and `unify` selects workspace-wide or per-member unification.

```toml
[build]
cargoToml = "Cargo.toml"
cargoLock = "Cargo.lock"
bloomeryLock = "bloomery.lock"   # omit to auto-detect
members = ["bloomery-cli"]       # omit to build every member
profileName = "release"
libPackages = false
devPackages = true
unify = true
```

## Toolchain

`[toolchain]` carries package references plus the linker selector. Package
references use the nixpkgs attribute-path encoding.

```toml
[toolchain]
rustc = "rustc"
clippy = "clippy"
cargo = "cargo"
lld = "lld"
mold = "mold"
stdenv = "stdenv"
linker = "lld"                   # "lld", "mold", "system", or omit
```

## Profiles

`[profile.release]` and `[profile.dev]` carry the scalar profile settings
`optLevel`, `lto`, `codegenUnits`, `panic`, `strip`, `debuginfo`, `targetCpu`,
`overflowChecks`, `linker`, and `linkArgs`.

```toml
[profile.release]
optLevel = 3
lto = "fat"
codegenUnits = 1

[profile.dev]
optLevel = 0
```

## Flags

`[flags]` carries the per-phase flag lists `rustc`, `test`, `clippy`, `doc`, and
`doctest`.

```toml
[flags]
rustc = ["-Copt-level=3"]
test = []
clippy = []
doc = ["-Dwarnings"]
doctest = []
```

## Development shell

`[devShell]` carries `enable`, `packages`, and `shellHook`. `packages` uses the
nixpkgs attribute-path encoding.

```toml
[devShell]
enable = true
packages = ["rust-analyzer", "bacon"]
shellHook = "echo ready"
```

## Checks

`[checks]` carries `enable`, `includePackageChecks`, `throwOnOutOfDate`,
`workspaceDependencies`, and `noDefaultFeatures`.

```toml
[checks]
enable = true
includePackageChecks = true
throwOnOutOfDate = false
workspaceDependencies = true
noDefaultFeatures = true
```

## Features

`[features]` carries `cratesIoIndex`.

```toml
[features]
cratesIoIndex = "crates-io-index"
```

## Formatters

`[formatters]` tunes the formatter set and its ordering graph. Each known
formatter, built in or supplied through the constructor's `extraFormatters`,
has an `enable` toggle and `before`/`after` lists of formatter names. Formatter
bodies carry Nix packages and stay in Nix; `config.toml` only enables and
orders them. An absent table enables the default set, including the `toml-sort`
formatter, with no ordering edges.

```toml
[formatters.toml-sort]
enable = true
before = ["taplo"]
after = []
```

See [Default formatter](formatters.md) for the resolved ordering graph and
its validation.

## Overrides

Overrides stay in Nix. Built-in sys-crate overrides, colocated `overrides.nix`
files, and the `overrides` argument accepted by `mkFlake` merge in the order
described in [Workspace options](workspace-options.md#overrides). They carry
arbitrary Nix values and are not represented in `config.toml`.

## Error handling

An absent configuration file, a syntactically invalid file, an option value of
the wrong type, an unknown key in a build table, a formatter ordering cycle, a
formatter self-reference, an ordering edge naming a formatter that is neither
built in nor supplied by the flake, and a package reference that does not
resolve in nixpkgs all fail evaluation with a `bloomery:`-prefixed message.
Bloomery never substitutes defaults for a broken configuration file.
