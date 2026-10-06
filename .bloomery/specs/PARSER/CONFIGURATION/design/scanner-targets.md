# Scanner targets

Scanner targets describe *where evidence may be found*, not what evidence
means. The scanners share a registry shape but retain source-specific parsing
and source locations.

| Scanner | Input | Configuration |
| --- | --- | --- |
| Rust | `.rs` files | `scanners.rust.paths` |
| Playwright | TypeScript/JavaScript files | `scanners.playwright.paths`, `tag_prefix` |
| Nix | `.nix` source files, including `*.test.nix` test files | `scanners.nix.paths`, `scanners.nix.testPaths` |

Glob expansion is deterministic and repository-relative. Paths outside the
configured repository are not accepted. A file that matches multiple patterns
is scanned with references de-duplicated by source span and requirement
identifier. Nix test discovery reads `scanners.nix.testPaths`; see the
[Nix scanner design](../../SCANNING/design/nix.md#test-discovery).

## Path globs

Configured source and test paths use a repository-relative glob grammar:

| Operator | Meaning |
| --- | --- |
| `*` | zero or more characters within one path segment |
| `?` | exactly one character within one path segment |
| `**` | zero or more path segments |

A file matched by several patterns is retained once, and expansion is
deterministic.

Every scanner is static: configuration selects source paths, scanners extract
references from source syntax or metadata, and no test command is executed.
The Nix scanner does not evaluate flakes; see the
[Nix scanner design](../../SCANNING/design/nix.md).
