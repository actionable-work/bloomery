# Configuration file

The configuration file is `.bloomery/config.toml`. It is required. Its paths
are intentionally relative so a repository can be moved without rewriting the
design tree.

## Required file

Every repository command requires `.bloomery/config.toml`. The only exemptions
are parser-generated help and the future `init` command, which bootstraps the
repository before configuration exists. A missing file is a
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
`[features]`. Those tables are owned by
[NIXLIB/FLAKE](../../../NIXLIB/FLAKE/design/configuration.md); the CLI parser
accepts them as valid configuration and does not interpret them as scanner or
spec target configuration.

## Resolution rules

- `specs.dir` is relative to `.bloomery/`.
- Rust, Playwright, and Nix paths are relative to the repository root.
- The Nix scanner extracts literal requirement IDs from `passthru.bloomery`
  metadata in matched Nix source files; it does not evaluate the flake.
- A disabled scanner contributes no references and performs no tool lookup.
- A configured scanner with an invalid path or option is a configuration error,
  not an empty scan.

## Error handling

A missing, unreadable, unparsable, or invalid configuration file produces a
`ConfigurationError`. Bloomery must not silently replace a broken file with
defaults. Parser help and `init` are the only commands that succeed without the
file.

## Loading order

Configuration parsing happens before workspace and source scanning. The parsed
configuration is immutable for the duration of a command so every pipeline
stage observes one consistent set of targets. The `[build]`-family tables are
carried through as an opaque section for the flake interface and do not change
scanner behavior.
