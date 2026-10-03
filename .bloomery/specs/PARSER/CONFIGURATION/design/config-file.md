# Configuration file

The configuration file is `.bloomery/config.toml`. It is optional. Its paths
are intentionally relative so a repository can be moved without rewriting the
design tree.

## Missing file defaults

When `.bloomery/config.toml` does not exist, Bloomery behaves as though it
contained an empty configuration and validates the resulting defaults:

- `specs.dir` is `"specs"`, resolving to `.bloomery/specs`.
- Rust, Playwright, and Nix scanners are disabled.
- Rust, Playwright, and Nix scanner path lists are empty.
- `scanners.playwright.tag_prefix` is `"@bloomery:"`.

An enabled scanner requires at least one repository-relative path pattern.

## Partial configuration

In a present file, every omitted table and key uses the same default as an
empty configuration. Defaults are applied before validation, so a partial file
is not treated differently from an explicitly written configuration with the
same effective values.

The configuration shape is:

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

## Resolution rules

- `specs.dir` is relative to `.bloomery/`.
- Rust, Playwright, and Nix paths are relative to the repository root.
- The Nix scanner extracts literal requirement IDs from `passthru.bloomery`
  metadata in matched Nix source files; it does not evaluate the flake.
- A disabled scanner contributes no references and performs no tool lookup.
- A configured scanner with an invalid path or option is a configuration error,
  not an empty scan.

## Error handling

Only a missing configuration file is optional. A present file that cannot be
read, cannot be parsed, or fails validation produces a `ConfigurationError`;
Bloomery must not silently replace a broken file with defaults.

## Workspace loading

Workspace discovery uses the default specs root when the configuration file is
absent. A valid `.bloomery/specs` tree therefore loads without requiring a
configuration file.

Configuration parsing happens before workspace and source scanning. The parsed
configuration is immutable for the duration of a command so every pipeline
stage observes one consistent set of targets.
