# Scanner targets

Scanner targets describe *where evidence may be found*, not what evidence
means. The scanners share a registry shape but retain source-specific parsing
and source locations.

| Scanner | Input | Configuration |
| --- | --- | --- |
| Rust | `.rs` files | `scanners.rust.paths` |
| Playwright | TypeScript/JavaScript files | `scanners.playwright.paths`, `tag_prefix` |
| Nix | Flake checks metadata | `scanners.nix.checks_attr`, `systems` |

Glob expansion is deterministic and repository-relative. Paths outside the
configured repository are not accepted. A file that matches multiple patterns
is scanned with references de-duplicated by source span and requirement
identifier.

The configuration layer does not infer targets from build outputs or execute
arbitrary commands. The Nix scanner is the sole exception to ordinary file
walking: it evaluates the configured metadata expression as described in the
[Nix scanner design](../../SCANNING/design/nix.md), without instantiating or
running derivations.
