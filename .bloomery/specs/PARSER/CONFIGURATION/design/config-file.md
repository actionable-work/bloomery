# Configuration file

The configuration file is `.bloomery/config.toml`. It is required. Its paths
are intentionally relative so a repository can be moved without rewriting the
design tree.

## Format versioning

The top-level integer `version` identifies the configuration's structural
layout. Format `2` is current, and every new or upgraded configuration declares
`version = 2`. Bloomery does not infer a version for an unversioned file or
accept format `0`. A missing version produces a `ConfigurationError` that asks
the user to set `version = 2`. `bloomery config upgrade` accepts format `1` and
migrates it to format `2`; other configuration readers require the current
format. The `version` field is format metadata, not a configurable
schema-catalog key.

The format version describes the structure of existing keys, not the set of
keys supported by the schema catalog. Adding a key does not require a format
version increase. Renaming, moving, nesting, or changing the representation of
an existing key requires a new format version. A migration from unversioned
files or format `0` is not defined.

## Required file

Every repository command requires `.bloomery/config.toml`. The only exemptions
are parser-generated help, the `init` command, which bootstraps the repository
before configuration exists, and the `config` command, whose mutating
operations create a missing file with a warning while its read-only operations
report defaults without writing. For every other command a missing file is a
`ConfigurationError`; Bloomery does not substitute defaults.

## Partial configuration

In a present file, every omitted table and key uses the documented default.
Defaults are applied before validation, so a partial file is not treated
differently from an explicitly written configuration with the same effective
values.

The scanner and spec configuration shape begins with the required format key:

```toml
version = 2

[specs]
dir = "specs"
# enable = true   # optional override; absent auto-detects from the specification tree

[scanners.rust]
enabled = true
paths = [
  "packages/rust/**/*.rs",
]

[scanners.playwright]
enabled = true
paths = [
  "packages/playwright/**/*.spec.ts",
  "packages/playwright/**/*.test.ts",
]
tag_prefix = "@bloomery:"

[scanners.nix]
enabled = true
paths = ["*.nix", "lib/**/*.nix", "nix/**/*.nix", "tests/**/*.nix"]
testPaths = ["**/*.test.nix"]
```

All three scanners are enabled by default, so an empty configuration scans
the default paths above. `scanners.nix.testPaths` selects the Nix test files
that [test discovery](../../SCANNING/design/test-discovery.md) treats as test
sites; an empty list disables Nix test-file discovery while `scanners.nix.paths`
still supplies `passthru.bloomery` references.

## Spec management enablement

Specification management is resolved from `specs.enable` and the presence of
the specification tree at `.bloomery/<specs.dir>/`:

| `specs.enable` | Specification tree | Result |
| --- | --- | --- |
| absent | present | enabled |
| absent | absent | disabled |
| `true` | present | enabled |
| `true` | absent | error |
| `false` | either | disabled |

An absent `specs.enable` auto-detects: a repository with a specification tree
has specification management, and one without it does not. An explicit value
overrides detection, so `specs.enable = false` disables management even when a
tree is present, and `specs.enable = true` requires the tree.

When specification management is disabled, the tree is neither required nor
read, the workspace resolves to an empty model, `bloomery check` omits the
static specification checks, and the requirement-management commands report
that specification management is disabled. Scanner keys remain independently
configurable but produce no references without a requirement model.

## Build tables

The same file carries Nix build tables `[build]`, `[build.toolchain]`,
`[build.profile.release]`, `[build.profile.dev]`, and `[build.flags]`, plus
`[devShell]`, `[checks]`, `[features]`, and `[formatters]`, and the
parameterized `[optimize]` family.
Their semantics and evaluation-time resolution are owned by
[NIXLIB/FLAKE](../../../NIXLIB/FLAKE/design/configuration.md). The CLI catalogs
the same keys and validates their types and enum value sets during
configuration loading, so a malformed build key fails before Nix evaluation;
the tables are still not interpreted as scanner or spec target configuration.

## Code health policy

`[checks.codeHealth]` is CLI-owned despite its parent table also carrying Nix
keys. It controls independently enabled health analysis, component weights,
score, function, and crate-size limits, baseline location, and allowed
regression. Defaults and validation live in
[code health configuration](code-health.md). The flake loader accepts this
subtable without generating a health derivation.

## Sub-flake composition

The `[flakes]` table is a parameterized family keyed by sub-flake name and is
consumed by the [NIXLIB/FLAKE composition design](../../../NIXLIB/FLAKE/design/composition.md).
Each `[flakes.<name>]` entry carries a required repository-relative `path`, a
`packages` boolean, an `apps` boolean, and a `checks` mode of `"none"`,
`"individual"`, or `"aggregate"`. The elevation keys default to off, so an
entry contributes nothing until a family is enabled.

```toml
[flakes.tests-basic]
path = "tests/basic-workspace"
packages = true
apps = true
checks = "individual"
```

## Resolution rules

- `specs.dir` is relative to `.bloomery/`.
- Rust, Playwright, and Nix paths, including `scanners.nix.testPaths`, are
  relative to the repository root.
- `flakes.<name>.path` is relative to the repository root and stays inside it.
- The Nix scanner extracts literal requirement IDs from `passthru.bloomery`
  metadata in files matched by `scanners.nix.paths` or
  `scanners.nix.testPaths`; it does not evaluate the flake.
- The Nix scanner treats files matched by `scanners.nix.testPaths` as test sites
  for candidate discovery; an empty list disables that discovery.
- A disabled scanner contributes no references and performs no tool lookup.
- A configured scanner with an invalid path or option is a configuration error,
  not an empty scan.

## Recommended default catalog

The [schema catalog](schema-catalog.md) is the single enumerated description of
every configurable option except the file-format metadata. It spans both the
CLI-owned tables and the Nix build tables, and pairs each option with a schema
type, documentation text, an effective default, and an optional upgrade
recommendation. The documentation text is the source for the
[`config document`](../../../CLI/CONFIG/design/documenting.md) comments.

The catalog is the source of truth for `bloomery config upgrade`, which
materializes absent upgrade recommendations, for `bloomery config` type
validation, and for the advisory
[SYNC recommendations](../../../CLI/SYNC/design/sync.md#recommendations), which
report a subset of absent keys without writing. Upgrade recommendations usually
equal the defaults applied to absent keys. Check-control keys are an
exception: they default to `false`, while upgrade recommends `true` to opt in.
Explicit values, including `false`, always take precedence over an upgrade.

## Error handling

A missing, unreadable, unparsable, or invalid configuration file produces a
`ConfigurationError`. A missing format version produces an error that asks the
user to add `version = 2` to `.bloomery/config.toml`. A malformed, negative, or
unsupported version, including `0` or `1` outside `config upgrade`, is also a
`ConfigurationError`; a version `1` error directs the user to
`bloomery config upgrade`. Bloomery must not silently replace a broken file
with defaults. Parser help, `init`, and `config` are the only commands that
succeed without the file; `config` creates it rather than treating its absence
as an error.

## Loading order

Configuration parsing requires format `2` and validates the full schema
catalog, including the build tables, before workspace and source scanning.
`config upgrade` migrates format `1` before validation and publication. The
parsed configuration is immutable for the duration of a command so every
pipeline stage observes one consistent set of targets. The `[build]`-family
tables are type-validated and then carried through for the flake interface;
they do not change scanner behavior.
