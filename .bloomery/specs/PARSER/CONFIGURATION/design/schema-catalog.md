# Configuration schema catalog

The schema catalog is the single enumerated description of every configurable
`.bloomery/config.toml` option. The top-level `version` field is format metadata
and is described by the
[configuration file design](config-file.md#format-versioning). The CLI uses the
catalog to address, validate, recommend, materialize, and document
configuration; the flake interface retains authority over build-table semantics
and evaluation-time resolution.

## Catalog fields

Each catalog entry carries an exact dotted key path, a schema type, an owning
feature, documentation text, an effective default when omission has a fixed
meaning, and an optional upgrade recommendation. Usually the recommendation
equals the effective default. Check control keys default to `false` but
recommend `true`, so an existing configuration keeps checks opt-in while
`config upgrade` explicitly sets supported check controls. The vulnerability
check has no upgrade recommendation because it requires a separately supplied
Nix input. Other keys whose omission is semantic are addressable but have no
recommendation:
`build.bloomeryLock` auto-detects, `build.members` means all members,
`build.toolchain.linker` is host-dependent, `features.cratesIoIndex` means
none, `specs.enable` auto-detects from the specification tree, and the
`build.profile.*` fields inherit from `Cargo.toml` and rustc.

`bloomery config upgrade` materializes recommended values only. Keys without a
recommendation remain absent unless the user sets them. `bloomery config
document` renders each entry's documentation text as a TOML comment on the
configured key, falling back to the line above when the value already carries a
trailing comment.

## Spec and scanner tables

Owned by [PARSER/CONFIGURATION](../README.md) and consumed by the CLI.

| Key | Type | Recommended default |
| --- | --- | --- |
| `specs.enable` | optional boolean | none |
| `specs.dir` | path inside `.bloomery/` | `"specs"` |
| `scanners.rust.enabled` | boolean | `true` |
| `scanners.rust.paths` | list of repository-relative globs | `["packages/rust/**/*.rs"]` |
| `scanners.playwright.enabled` | boolean | `true` |
| `scanners.playwright.paths` | list of repository-relative globs | `["packages/playwright/**/*.spec.ts", "packages/playwright/**/*.test.ts"]` |
| `scanners.playwright.tag_prefix` | non-empty string | `"@bloomery:"` |
| `scanners.nix.enabled` | boolean | `true` |
| `scanners.nix.paths` | list of repository-relative globs | `["*.nix", "lib/**/*.nix", "nix/**/*.nix", "tests/**/*.nix"]` |
| `scanners.nix.testPaths` | list of repository-relative globs | `["**/*.test.nix"]` |

`specs.enable` overrides specification-management auto-detection. When it is
absent, a repository has specification management exactly when its
specification tree exists; `true` requires the tree and `false` disables
management. Because omission is semantic, it carries no recommended default.
See the [configuration file design](config-file.md#spec-management-enablement).

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

### `[build.toolchain]`

| Key | Type | Recommended default |
| --- | --- | --- |
| `build.toolchain.rustc` | nixpkgs attribute path | `"rustc"` |
| `build.toolchain.clippy` | nixpkgs attribute path | `"clippy"` |
| `build.toolchain.cargo` | nixpkgs attribute path | `"cargo"` |
| `build.toolchain.lld` | nixpkgs attribute path | `"lld"` |
| `build.toolchain.mold` | nixpkgs attribute path | `"mold"` |
| `build.toolchain.stdenv` | nixpkgs attribute path | `"stdenv"` |
| `build.toolchain.linker` | `"lld"`, `"mold"`, `"system"`, or absent | none |

### `[build.profile.release]` and `[build.profile.dev]`

Both sub-tables share the same ten keys. Every profile key has no recommended
default because omission delegates to `Cargo.toml` and rustc defaults.

| Key | Type |
| --- | --- |
| `build.profile.<name>.optLevel` | integer `0`-`3` or `"0"`, `"1"`, `"2"`, `"3"`, `"s"`, `"z"` |
| `build.profile.<name>.lto` | boolean or `"fat"`, `"thin"`, `"off"`, `"full"`, `"none"`, `"yes"`, `"no"` |
| `build.profile.<name>.codegenUnits` | positive integer |
| `build.profile.<name>.panic` | `"unwind"` or `"abort"` |
| `build.profile.<name>.strip` | boolean or `"none"`, `"debuginfo"`, `"symbols"` |
| `build.profile.<name>.linker` | string |
| `build.profile.<name>.linkArgs` | list of strings |
| `build.profile.<name>.targetCpu` | string |
| `build.profile.<name>.debuginfo` | boolean, integer `0`-`2`, or `"0"`, `"1"`, `"2"`, `"none"`, `"line-directives-only"`, `"line-tables-only"`, `"limited"`, `"full"` |
| `build.profile.<name>.overflowChecks` | boolean |

### `[build.flags]`

| Key | Type | Recommended default |
| --- | --- | --- |
| `build.flags.rustc` | list of strings | `["-Copt-level=3"]` |
| `build.flags.test` | list of strings | `[]` |
| `build.flags.clippy` | list of strings | `[]` |
| `build.flags.doc` | list of strings | `["-Dwarnings"]` |
| `build.flags.doctest` | list of strings | `[]` |

### `[devShell]`

| Key | Type | Recommended default |
| --- | --- | --- |
| `devShell.enable` | boolean | `true` |
| `devShell.packages` | list of nixpkgs attribute paths | `[]` |
| `devShell.shellHook` | string | `""` |

### `[checks]`

| Key | Type | Upgrade recommendation |
| --- | --- | --- |
| `checks.enable` | boolean | `true` |
| `checks.crates.enable` | boolean | `true` |
| `checks.packages.enable` | boolean | `true` |
| `checks.lockFreshness.enable` | boolean | `true` |
| `checks.workspaceDependencies.enable` | boolean | `true` |
| `checks.noDefaultFeatures.enable` | boolean | `true` |
| `checks.vulnerabilities.enable` | boolean | none |
| `checks.vulnerabilities.ignoreAdvisories` | list of RustSec advisory IDs | `[]` |
| `checks.vulnerabilities.informational.unmaintained` | `warn`, `deny`, or `ignore` | `warn` |
| `checks.vulnerabilities.informational.unsound` | `warn`, `deny`, or `ignore` | `warn` |
| `checks.vulnerabilities.informational.notice` | `warn`, `deny`, or `ignore` | `warn` |
| `checks.vulnerabilities.informational.unknown` | `warn`, `deny`, or `ignore` | `warn` |
| `checks.bans.enable` | boolean | `true` |
| `checks.bans.allow` | list of package selectors | `[]` |
| `checks.bans.deny` | list of package selectors | `[]` |
| `checks.bans.exceptions` | list of package selectors | `[]` |
| `checks.licenses.enable` | boolean | `true` |
| `checks.licenses.allow` | list of SPDX license IDs or exception pairs | `[]` |
| `checks.licenses.deny` | list of SPDX license IDs or exception pairs | `[]` |
| `checks.licenses.exceptions` | list of package selectors | `[]` |
| `checks.licenses.overrides.<package-selector>` | SPDX expression | none |

When omitted, every check `enable` setting in this table is effectively
`false`. Every check enable setting has an upgrade recommendation of `true`
except `checks.vulnerabilities.enable`, which has no recommendation because it
requires the separately supplied `rustsecAdvisoryDb` Nix input. `checks.enable`
gates generated Nix checks. `checks.lockFreshness.enable` controls the stale-lock
policy independently. Package selectors are canonical crate names or exact
`name@version` values. A non-empty package or license `allow` list enables a
whitelist; an empty list imposes no allowlist. The license override family is
keyed by package selector and has no upgrade recommendation.

The vulnerability, license, and package-ban policy values are consumed by the
per-member Nix checks in
[Dependency policy checks](../../../NIXLIB/GRAPH/design/dependency-policy-checks.md).
The RustSec advisory database path is a `mkFlake` Nix argument, not a
config.toml key.

`checks.specifications`, `checks.codeHealth`, and
`checks.clippySuppressions` are CLI-owned nested tables. Their `enable` keys
default to `false` and have upgrade recommendations of `true`; their policy
designs define their scope. The Nix loader validates and accepts these tables
without generating corresponding Nix checks.

An unupgraded configuration generates no Nix checks by default. `config upgrade`
sets each missing check enable setting with a recommendation to `true` unless a
value was already configured.
The
[CLI build command](../../../CLI/BUILD/README.md) materializes production
packages instead. Setting `checks.packages.enable = true` adds the release
`<bin>:bin` and `<crate>:lib` checks to the check set. See the
[flake check output](../../../NIXLIB/FLAKE/design/output-contract.md#checks).

### `[checks.specifications]`

Owned by [CLI/CHECK](../../../CLI/CHECK/design/pipeline.md) and parsed by the
CLI. Its `enable` key defaults to `false` and has an upgrade recommendation of
`true`. It gates specification structure and traceability checks when
specification management is active. The Nix loader accepts the table without
generating a corresponding Nix check.

### `[checks.codeHealth]`

Owned by [CLI/CHECK](../../../CLI/CHECK/design/code-health.md) and parsed by the
CLI. The [code health catalog](code-health.md#catalog) enumerates every nested
key, schema type, constraint, and recommended default. These entries participate
in config get/set/unset/list/upgrade/document like all other fixed keys. Its
`enable` key defaults to `false` and has an upgrade recommendation of `true`.
The Nix loader validates and accepts the table without creating a health
derivation.

### `[checks.clippySuppressions]`

Owned by [CLI/CHECK](../../../CLI/CHECK/design/clippy-suppressions.md) and
parsed by the CLI. The table enables the inline Clippy suppression check and
lists the exact Clippy lint names permitted as exceptions. Its `enable` key
defaults to `false` and has an upgrade recommendation of `true`. The Nix loader
accepts the table without creating a Clippy suppression derivation.

### `[features]`

| Key | Type | Recommended default |
| --- | --- | --- |
| `features.cratesIoIndex` | repository-relative path, absent means none | none |

### `[optimize]`

Owned by
[NIXLIB/OPTIMIZE](../../../NIXLIB/OPTIMIZE/design/optimization.md#opt-in-configuration)
and catalogued for CLI editing and validation. `[optimize]` is a parameterized
name family: the binary names are repository-defined, so their keys carry no
recommendation. Every name has the same sub-table shape:

| Key | Type | Recommended default |
| --- | --- | --- |
| `optimize.<name>.enable` | boolean | none |
| `optimize.<name>.script` | repository-relative path | none |
| `optimize.<name>.systems` | table keyed by system name | none |
| `optimize.<name>.systems.<system>.targetCpu` | string | none |
| `optimize.<name>.pgo` | table | none |
| `optimize.<name>.pgo.enable` | boolean | none |
| `optimize.<name>.pgo.scope` | `workspace` or `all` | none |
| `optimize.<name>.bolt` | table | none |
| `optimize.<name>.bolt.enable` | boolean | none |
| `optimize.<name>.bolt.functions` | boolean | none |
| `optimize.<name>.bolt.blocks` | boolean | none |

`script` is required for an enabled entry when a training stage is enabled and
stays inside the repository root.
The per-system `targetCpu` defaults to the active profile's target CPU. A
non-empty `systems` table restricts the optimized build to the listed systems. The
`pgo` and `bolt` tables select the PGO and
BOLT stages independently, and `pgo.scope` chooses whether PGO rebuilds the
workspace crates or the whole dependency closure. Setting a per-system
`targetCpu` tunes the final binary even when both stages are disabled. The
table is absent by default and builds no optimized binaries. An enabled entry
replaces the named binary's exported package and release app. Pipeline and
validation semantics are owned by the linked design.

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

### `[flakes]`

Owned by [NIXLIB/FLAKE](../../../NIXLIB/FLAKE/design/composition.md) and
catalogued for CLI editing and validation. `[flakes]` is a parameterized name
family: the sub-flake names are repository-defined, so their keys carry no
recommendation. Every name has the same sub-table shape:

| Key | Type | Recommended default |
| --- | --- | --- |
| `flakes.<name>.path` | repository-relative path | none |
| `flakes.<name>.packages` | boolean | none |
| `flakes.<name>.apps` | boolean | none |
| `flakes.<name>.checks` | `"none"`, `"individual"`, or `"aggregate"` | none |

`path` is required when a sub-flake entry is present. No key for any other
output family is catalogued, so a request to elevate one is an unknown-key
error.

## Validation

The CLI validates every catalogued key's TOML type, including the union and
enum value sets, and rejects unknown keys in a build table or a parameterized
family. Validation happens before any consuming command runs, so a build-table
typo fails without Nix evaluation. Nix remains authoritative for
package-attribute resolution and for the evaluation-time semantics of each
option; a string that is well-typed but does not resolve in nixpkgs still fails
at evaluation.

## Versioning

The catalog ships with the Bloomery binary. A newer binary may add entries, and
`bloomery config upgrade` materializes any newly recommended entries while
leaving present values untouched. Explicit values, including those
materialized by upgrade, always take precedence and suppress advisory
recommendations. Catalog additions are independent of the configuration format
version: a new key uses its documented default when absent and does not require
a format migration.
