# Public static reports

`bloomery_check::report::{analyze_tree, analyze, StaticReport}` composes the
repository specification loader, syntactic scanners and relational validator.
It requires no Platform files, hosted services or execution infrastructure.

```rust
use bloomery_check::report::{analyze_tree, StageState};
use bloomery_model::Config;
use bloomery_workspace::source::MemoryTree;

let tree = MemoryTree::new([("tests/example.rs", "#[test] fn example() {}")])?;
let mut config = Config::default();
config.scanners.rust.paths = vec!["tests/**/*.rs".into()];
let report = analyze_tree(&tree, config);
assert_eq!(report.version, 2);
assert_eq!(report.completeness.tree, StageState::Complete);
assert_eq!(report.tests[0].name.as_deref(), Some("example"));
```

Use `analyze(root, config)` for filesystem input. Both functions return the same
report for equivalent trees and explicit configuration, including relative
locations, diagnostics, literal references and test sites. Configuration and
analysis errors are report findings. The CLI adapter is
`bloomery --json spec trace --report`; its payload is `result.report` under
operation `trace_report`. CLI flake/configuration preflights still apply.
Exit zero means the report was produced, even when it contains findings.

Report format 2 adds optional `end_line` and `end_column` to each location.
Coordinates use one-based Unicode scalar positions and exclusive ends; tabs count
as one scalar and LF advances the line, including in CRLF input. Unknown columns
and ends remain null. References cover quoted literals, Rust test sites cover
function names, Playwright sites cover calls, and Nix sites cover whole files.
The parser contract is `bloomery-static-syntax-v2`; specification schema 1 is
unchanged. Rust ingestion errors without retained spans remain line-only.

Reports bind the complete typed configuration, tree snapshot, active specification
subtree, parser contract and specification schema. `identity` is SHA-256 of the
canonical report payload with that field omitted. Canonical JSON uses compact
UTF-8 encoding and sorted object keys. The tree manifest has sorted `files`
(`path`, `sha256:<hex>` pairs) and `directories` (including inferred and empty
directories). The specification manifest selects that same structure beneath the
active root, or is empty when specifications are disabled. A failed stage has no
successful input identity. Contract constants are exported from `report`.

Analysis snapshots every indexed file once. Use `MemoryTree::from_bytes` or
override `SourceTree::read_bytes` when binary files are present. Only selected
documents require UTF-8. Shared exclusions (`.git`, `.jj`, `target`, `.direnv`,
`result`, `node_modules`) and filesystem symlinks are outside this tree identity.
The filesystem must remain stable while the snapshot is collected; it is not an
atomic Git snapshot or a sandbox for hostile filesystems.

`completeness` tracks configuration, tree, specifications, discovery and relations
separately. Disabled/no-match scanners make discovery partial. Absent bindings
then remain unresolved; complete discovery can report missing tests. Aggregate
scanner failure returns no partial facts, so enabled scanners are unavailable and
relations remain unattempted. Disabled or invalid specs likewise prevent relational
validation while independent discovery remains available. Findings do not imply
that a stage was incomplete.

Scope is explicitly syntactic. Targets, features and conditional compilation
resolution are unsupported. Reports do not execute tests or contain execution
outcomes, attestations or hosted approval. Specs are discovered solely from
repository files; lifecycle tools edit those files. Revision/lineage metadata and
explicit legacy migration are outside this report format. APIs are experimental
and the crate remains version 0.1.0 under `MIT OR Apache-2.0`.
