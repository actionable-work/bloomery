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
assert_eq!(report.version, 5);
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

Report format 5 retains optional `end_line` and `end_column` on each location.
Coordinates use one-based Unicode scalar positions and exclusive ends; tabs count
as one scalar and LF advances the line, including in CRLF input. Unknown columns
and ends remain null. References cover quoted literals, Rust test sites cover
function names, Playwright sites cover calls, and Nix sites cover whole files.
The parser contract is `bloomery-static-syntax-v5`; specification schema 1 is
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

Default scope is syntactic; explicit local Rust predicate scope is described below.
Reports do not execute tests or contain execution
outcomes, attestations or hosted approval. Specs are discovered solely from
repository files; lifecycle tools edit those files. Revision/lineage metadata and
explicit legacy migration are outside this report format. APIs are experimental
and the crate remains version 0.1.0 under `MIT OR Apache-2.0`.

Use `report::analyze_tree_with_cfg(&tree, config, cfg)` or
`report::analyze_with_cfg(root, config, cfg)` for explicit local Rust predicates:

```rust
use bloomery_check::report::RustCfg;
let cfg = RustCfg::parse_json(r#"{"features":["fast"],"flags":{"test":true},"values":{"target_os":["linux"]},"target":"x86_64-unknown-linux-gnu"}"#)?;
```

The equivalent CLI is `bloomery --json spec trace --report --rust-cfg JSON`.
Unknown keys or invalid shapes are usage errors. Features are caller-supplied
effective features: omitted/null means unknown; an empty array means none.
Missing flags or named-value keys are unknown; explicit false flags and empty
value sets are known false. The target label never supplies implicit facts.

`scope.rust_cfg` contains normalized context, and `inputs.scope` hashes the full
scope. Each Rust test/binding has a `rust_cfg` condition annotation with
`satisfied`, `rejected` or `unresolved` state and sorted unresolved reasons.
Without cfg scope, these annotations are null and discovery stays syntactic.
The scanner also exposes owned `CfgExpression` / `RustCondition` facts through
`scan_tree_with_conditions`, using the existing source ingestion cache.

Report/parser version 5 includes this scope, annotations and `completeness.rust_cfg`;
specification schema 1 is unchanged. Only satisfied Rust conditions contribute
scoped coverage. All literal references and orphan/manual findings remain
visible. Relations stay partial and absent bindings unresolved. Local cfg
completeness never establishes Cargo feature unification, required-features
targets, module ancestry or macro expansion. These limitations remain explicit
in scope; no compiler, Cargo metadata or environment probing runs.

For an explicitly declared integration-test entrypoint, add a selector:

```rust
let cfg = RustCfg::parse_json(r#"{"features":["fast"],"flags":{"test":true},"cargo_test":{"manifest":"pkg/Cargo.toml","name":"smoke"}}"#)?;
```

The selected `[[test]]` must have explicit `name` and `path` fields. The report's
`cargo_test` projection contains the canonical entrypoint, sorted required
features, harness mode and eligibility. `completeness.rust_target` tracks this
projection independently. An absent effective feature set leaves nonempty
requirements unresolved; custom harnesses are also unresolved.

Each Rust test/binding receives a separate `rust_target` condition. Tests in the
entrypoint and known direct out-of-line children may credit Rust
coverage when both cfg and target conditions are satisfied. Other files have
unresolved module membership; failed target projection likewise blocks Rust
coverage while discovery and literal findings continue. Disabled Rust scanning
disables projection.

Manifest paths are canonical repository-relative Cargo.toml paths. Entrypoint
paths resolve relative to that manifest, allowing dot segments within the tree.
Missing, escaping, malformed, duplicate or inferred entrypoints are findings.
This is a declared-entrypoint projection, not Cargo manifest validation, target
auto-discovery or complete module resolution. No Cargo process runs.
Specification schema 1 and repository-only specification discovery are unchanged.

Selected Cargo tests also expose `cargo_test.modules` and
`completeness.rust_modules`. Direct `mod child;` and literal `#[path]`
declarations resolve against the supplied tree. Child target conditions include
entrypoint and declaration cfg gates; local cfg stays separate. Alternative
routes to one file combine by any satisfied route. Scanner globs still determine
which tests are discovered; a root outside those globs supplies context only.

Missing/ambiguous, escaping/self-referential or unsupported paths produce located
`RustModuleResolutionError` findings and unresolved membership. Rejected gates
suppress irrelevant path findings. Module records retain declaration spans,
canonical candidate paths, resolved paths and conditions. Direct projection has
its own partial/failed/disabled/not-attempted states. Transitive membership,
out-of-line children below inline modules and conditional paths remain outside
scope. Relations stay partial; static results cannot establish compilation or
execution success. Report/parser contract 5 retains specification schema 1.
