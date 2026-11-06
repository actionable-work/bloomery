# Public static discovery

`bloomery-scanning` (`bloomery_scanning` in Rust), version 0.1.0, is a public
library under `MIT OR Apache-2.0`. It requires no Platform files or services.
It shares the source parsers used by `bloomery spec trace`, `spec candidates`
and static specification checking. Its interfaces remain experimental.

```rust
use bloomery_model::config::ScannersConfig;
use bloomery_scanning::{MemoryTree, scan_tree};

let tree = MemoryTree::new([(
    "tests/example.rs",
    "#[cfg_attr(any(), bloomery(\"APP-CORE-BEHAVIOR-001\"))]\n\
     #[test]\nfn example() { panic!(\"never executed\"); }",
)])?;
let mut config = ScannersConfig::default();
config.rust.paths = vec!["tests/**/*.rs".into()];
config.playwright.enabled = false;
config.nix.enabled = false;
let analysis = scan_tree(&tree, &config).map_err(|errors| format!("{errors:?}"))?;
assert_eq!(analysis.output.tests[0].references, ["APP-CORE-BEHAVIOR-001"]);
assert!(!analysis.is_complete()); // Two scanners are explicitly disabled.
```

For a local repository, replace `tree` with `FilesystemTree::new(root)`.
The adapters are owned by `bloomery_workspace::source` and re-exported here.
Implement `SourceTree` for other caller-authorized stores; return canonical
repository-relative UTF-8 paths from `paths()` and content from `read(path)`.
The index and contents must stay stable during a call. MemoryTree construction
rejects noncanonical paths, duplicates and file/directory collisions.
Filesystem discovery skips `.git`, `.jj`, `target`, `.direnv`, `result`, `node_modules`
and symbolic links. It is a local adapter, not a sandbox for hostile live filesystems.

`scan_tree` returns relative locations and deterministically ordered facts.
`ScanAnalysis.scope` is `ScanScope::Syntactic`. Each scanner reports
`DiscoveryState::{Disabled, NoMatchingFiles, Complete}`. Failed input or parsing
returns diagnostics and no successful analysis. `is_complete()` requires all
three scanners to complete syntactic discovery; it makes no coverage claim.
Rust cfg conditions, Cargo target/feature resolution, semantic TypeScript call
resolution and Nix evaluation are outside this scope. Requirement IDs remain
unresolved literal bindings until checked against specifications.

`scan_all(&Context)` and `scan_all_with_session(&Context, &AnalysisSession)`
preserve filesystem source locations rooted at `Context.root`; the latter shares
Rust ingestion with code-health consumers. Their `ScanOutput` projection does
not expose discovery states. Existing CLI commands preserve their flags, JSON
shapes and exit codes: 0 for a successful projection, 1 for load/scan failures,
2 for invalid usage. Command success indicates a projection was produced;
it does not mean tests passed or authorize hosted approval.

The standalone consumer in `tests/static-scanning-parity/consumer.rs` is compiled
and compared with the actual CLI by the `core:static-scanning-parity` Nix check.
It compares bindings, locations, ordering, syntactic scope and discovery states
between library adapters and the fields available in the CLI projections.

The [versioned static report API](../check/README.md) composes these scanners
with tree/configuration/spec-set identities and explicit completeness. Static spec parsing through
`bloomery_workspace::load_tree` accepts the same tree and explicit configuration.
Default/custom `.bloomery/<specs.dir>` paths and duplicate-ID validation belong
to `bloomery-workspace`; configuration-format upgrades do not
constitute a legacy specification-tree migration or preserve spec revision and
lineage metadata. Target/feature resolution and explicit legacy migration remain
separate public contract work.
