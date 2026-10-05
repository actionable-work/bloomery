# Configuration schema catalog

The schema catalog is the single enumerated description of every recognized
`.bloomery/config.toml` key. The CLI uses it to address, validate, recommend,
materialize, and document configuration; the flake interface retains authority
over build-table semantics and evaluation-time resolution.

## Catalog fields

Each catalog entry carries an exact dotted key path, a schema type, an owning
feature, documentation text, and an optional recommended default. A recommended
default is present only when omitting the key has a single fixed
Bloomery-defined effective value. Keys whose omission is semantic are
addressable but have no recommendation:
`build.bloomeryLock` auto-detects, `build.members` means all members,
`toolchain.linker` is host-dependent, `features.cratesIoIndex` means none, and
the `profile.*` fields inherit from `Cargo.toml` and rustc.

`bloomery config upgrade` materializes recommended keys only. Keys without a
recommendation remain absent unless the user sets them. `bloomery config
document` renders each entry's documentation text as a TOML comment on the
configured key, falling back to the line above when the value already carries a
trailing comment.

## Spec and scanner tables

Owned by [PARSER/CONFIGURATION](../README.md) and consumed by the CLI.

| Key | Type | Recommended default |
| --- | --- | --- |
| `specs.dir` | path inside `.bloomery/` | `"specs"` |
| `scanners.rust.enabled` | boolean | `false` |
| `scanners.rust.paths` | list of repository-relative globs | `[]` |
| `scanners.playwright.enabled` | boolean | `false` |
| `scanners.playwright.paths` | list of repository-relative globs | `[]` |
| `scanners.playwright.tag_prefix` | non-empty string | `"@bloomery:"` |
| `scanners.nix.enabled` | boolean | `false` |
| `scanners.nix.paths` | list of repository-relative globs | `[]` |

## Build tables

Owned by [NIXLIB/FLAKE](../../../NIXLIB/FLAKE/README.md) and catalogued for CLI
editing and validation.

### `[build]`

| Key | Type | Recommended default |
| --- | --- | --- |
| `build.cargoToml` | repository-relative path | `"Cargo.toml"` |
| `build.cargoLock` | repository-relative path | `"Cargo.lock"` |
| `build.bloomeryLock` | repository-relative path, absent means auto-detect | none |
| `build.members` | list of crate names, absent means all | none |
| `build.profileName` | string | `"release"` |
| `build.libPackages` | boolean | `false` |
| `build.devPackages` | boolean | `false` |
| `build.unify` | boolean | `true` |

### `[toolchain]`

| Key | Type | Recommended default |
| --- | --- | --- |
| `toolchain.rustc` | nixpkgs attribute path | `"rustc"` |
| `toolchain.clippy` | nixpkgs attribute path | `"clippy"` |
| `toolchain.cargo` | nixpkgs attribute path | `"cargo"` |
| `toolchain.lld` | nixpkgs attribute path | `"lld"` |
| `toolchain.mold` | nixpkgs attribute path | `"mold"` |
| `toolchain.stdenv` | nixpkgs attribute path | `"stdenv"` |
| `toolchain.linker` | `"lld"`, `"mold"`, `"system"`, or absent | none |

### `[profile.release]` and `[profile.dev]`

Both sub-tables share the same ten keys. Every profile key has no recommended
default because omission delegates to `Cargo.toml` and rustc defaults.

| Key | Type |
| --- | --- |
| `profile.<name>.optLevel` | integer `0`-`3` or `"0"`, `"1"`, `"2"`, `"3"`, `"s"`, `"z"` |
| `profile.<name>.lto` | boolean or `"fat"`, `"thin"`, `"off"`, `"full"`, `"none"`, `"yes"`, `"no"` |
| `profile.<name>.codegenUnits` | positive integer |
| `profile.<name>.panic` | `"unwind"` or `"abort"` |
| `profile.<name>.strip` | boolean or `"none"`, `"debuginfo"`, `"symbols"` |
| `profile.<name>.linker` | string |
| `profile.<name>.linkArgs` | list of strings |
| `profile.<name>.targetCpu` | string |
| `profile.<name>.debuginfo` | boolean, integer `0`-`2`, or `"0"`, `"1"`, `"2"`, `"none"`, `"line-directives-only"`, `"line-tables-only"`, `"limited"`, `"full"` |
| `profile.<name>.overflowChecks` | boolean |

### `[flags]`

| Key | Type | Recommended default |
| --- | --- | --- |
| `flags.rustc` | list of strings | `["-Copt-level=3"]` |
| `flags.test` | list of strings | `[]` |
| `flags.clippy` | list of strings | `[]` |
| `flags.doc` | list of strings | `["-Dwarnings"]` |
| `flags.doctest` | list of strings | `[]` |

### `[devShell]`

| Key | Type | Recommended default |
| --- | --- | --- |
| `devShell.enable` | boolean | `true` |
| `devShell.packages` | list of nixpkgs attribute paths | `[]` |
| `devShell.shellHook` | string | `""` |

### `[checks]`

| Key | Type | Recommended default |
| --- | --- | --- |
| `checks.enable` | boolean | `true` |
| `checks.includePackageChecks` | boolean | `true` |
| `checks.throwOnOutOfDate` | boolean | `false` |
| `checks.workspaceDependencies` | boolean | `true` |
| `checks.noDefaultFeatures` | boolean | `true` |

### `[features]`

| Key | Type | Recommended default |
| --- | --- | --- |
| `features.cratesIoIndex` | repository-relative path, absent means none | none |

### `[formatters]`

Owned by [NIXLIB/FLAKE](../../../NIXLIB/FLAKE/README.md) and catalogued for CLI
editing and validation, alongside the build tables. `[formatters]` tunes
Bloomery's formatter set. The catalog describes a parameterized formatter-name
family rather than a fixed list of names, because the flake can add formatters
beyond the built-in set. Every name has the same sub-table shape:

| Key | Type | Recommended default |
| --- | --- | --- |
| `formatters.<name>.enable` | boolean | `true` |
| `formatters.<name>.before` | list of formatter names | `[]` |
| `formatters.<name>.after` | list of formatter names | `[]` |

The built-in formatter names (`deadnix`, `alejandra`, `rustfmt`, `shfmt`,
`shellcheck`, `taplo`, `toml-sort`, and `yamlfmt`) have Bloomery-defined bodies
and carry the recommended defaults above. A name outside that set is valid only
when the flake supplies an
[extra formatter body](../../../NIXLIB/FLAKE/design/formatters.md#custom-formatters),
and its keys carry no recommendation because the flake decides which names
exist. `before` and `after` are ordering edges: the builder resolves them into a
total order and rejects cycles, self-references, and names that are neither
built in nor flake-supplied. Referencing a known formatter that is disabled is
allowed and the edge is ignored.

## Validation

The CLI validates every catalogued key's TOML type, including the union and
enum value sets, and rejects unknown keys in a build table. Validation happens
before any consuming command runs, so a build-table typo fails without Nix
evaluation. Nix remains authoritative for package-attribute resolution and for
the evaluation-time semantics of each option; a string that is well-typed but
does not resolve in nixpkgs still fails at evaluation.

## Versioning

The catalog ships with the Bloomery binary. A newer binary may add entries, and
`bloomery config upgrade` materializes any newly recommended entries while
leaving present values untouched. Explicit values, including those
materialized by upgrade, always take precedence and suppress advisory
recommendations.
