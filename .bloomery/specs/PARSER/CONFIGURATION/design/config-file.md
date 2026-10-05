# Configuration file

The configuration file is `.bloomery/config.toml`. It is required. Its paths
are intentionally relative so a repository can be moved without rewriting the
design tree.

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

The scanner and spec configuration shape is:

```toml
[specs]
dir = "specs"

[scanners.rust]
enabled = true
paths = [
  "crates/*/src/**/*.rs",
  "crates/*/tests/**/*.rs",
  "tests/**/*.rs",
]

[scanners.playwright]
enabled = true
paths = [
  "e2e/**/*.spec.ts",
  "frontend/tests/**/*.test.ts",
]
tag_prefix = "@bloomery:"

[scanners.nix]
enabled = true
paths = ["*.nix", "nix/**/*.nix", "tests/**/*.nix"]
```

## Build tables

The same file carries the Nix build tables `[build]`, `[toolchain]`,
`[profile.release]`, `[profile.dev]`, `[flags]`, `[devShell]`, `[checks]`, and
`[features]`. Their semantics and evaluation-time resolution are owned by
[NIXLIB/FLAKE](../../../NIXLIB/FLAKE/design/configuration.md). The CLI catalogs
the same keys and validates their types and enum value sets during
configuration loading, so a malformed build key fails before Nix evaluation;
the tables are still not interpreted as scanner or spec target configuration.

## Resolution rules

- `specs.dir` is relative to `.bloomery/`.
- Rust, Playwright, and Nix paths are relative to the repository root.
- The Nix scanner extracts literal requirement IDs from `passthru.bloomery`
  metadata in matched Nix source files; it does not evaluate the flake.
- A disabled scanner contributes no references and performs no tool lookup.
- A configured scanner with an invalid path or option is a configuration error,
  not an empty scan.

## Recommended default catalog

The [schema catalog](schema-catalog.md) is the single enumerated description of
every recognized configuration key. It spans both the CLI-owned tables and the
Nix build tables, and pairs each key with a schema type and, when omission has a
single fixed effective value, a recommended default.

The catalog is the source of truth for `bloomery config upgrade`, which
materializes absent recommended keys, for `bloomery config` type validation, and
for the advisory
[SYNC recommendations](../../../CLI/SYNC/design/sync.md#recommendations), which
report a subset of absent keys without writing. Catalog values equal the
defaults applied to an absent key, so materializing them does not change the
effective configuration. Explicit values, including materialized ones, always
take precedence.

## Error handling

A missing, unreadable, unparsable, or invalid configuration file produces a
`ConfigurationError`. Bloomery must not silently replace a broken file with
defaults. Parser help, `init`, and `config` are the only commands that succeed
without the file; `config` creates it rather than treating its absence as an
error.

## Loading order

Configuration parsing happens before workspace and source scanning and
validates the full schema catalog, including the build tables. The parsed
configuration is immutable for the duration of a command so every pipeline
stage observes one consistent set of targets. The `[build]`-family tables are
type-validated and then carried through for the flake interface; they do not
change scanner behavior.
