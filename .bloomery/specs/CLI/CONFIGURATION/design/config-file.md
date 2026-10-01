# Configuration file

The configuration file is `.bloomery/config.toml`. Its paths are intentionally
relative so a repository can be moved without rewriting the design tree.

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
checks_attr = ".#checks"
systems = ["x86_64-linux", "aarch64-darwin"]
```

## Resolution rules

- `specs.dir` is relative to `.bloomery/`.
- Rust and Playwright paths are relative to the repository root.
- `checks_attr` identifies the flake output passed to the Nix scanner.
- If Nix `systems` is omitted, the host system is used.
- A disabled scanner contributes no references and performs no tool lookup.
- A configured scanner with an invalid path or option is a configuration error,
  not an empty scan.

Configuration parsing happens before workspace and source scanning. The parsed
configuration is immutable for the duration of a command so every pipeline
stage observes one consistent set of targets.
