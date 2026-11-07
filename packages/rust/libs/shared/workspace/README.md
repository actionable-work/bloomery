# Static repository specifications

`bloomery-workspace` discovers specifications solely from repository files.
The loader does not consult a hosted registry or manage specification lifecycle.
Hosted tools edit the repository's specifications and analyze the resulting tree.
The library remains experimental, version 0.1.0, under `MIT OR Apache-2.0`.

```rust
use bloomery_model::Config;
use bloomery_workspace::{load_tree, source::MemoryTree};

// `files` contains authorized repository-relative paths and UTF-8 content.
let tree = MemoryTree::new(files).expect("canonical repository paths");
let context = load_tree(&tree, Config::default()).expect("valid specifications");
```

`load_tree(&dyn SourceTree, Config)` validates explicit configuration and applies
the same hierarchy, Markdown, TOML, ID and design-link rules as `load(root)`.
Virtual loading needs no configuration file, filesystem, process, network or
Platform dependency. It reads exactly `.bloomery/<specs.dir>` as its active root.
It does not merge legacy roots. Explicit `specs.enable = false` skips tree access;
otherwise enablement is resolved against the supplied tree and retained in
`Context.specs_enabled`.

`load_tree` returns relative paths and an empty `Context.root`; `load(root)` reads
the configuration file, calls the same implementation and roots domain paths and
diagnostic locations at `root`. Diagnostic messages and notes use relative paths.
`SourceTree` supplies files, content and optional explicit directories. File
ancestors are inferred. Use `MemoryTree::with_directories(files, directories)`
when empty directories matter. `TreeIndex` validates collisions and supplies
deterministic path queries. `FilesystemTree` excludes symbolic links and standard
ignored directories; it is an adapter for stable local content, not a sandbox.

Tree adapters now live in `bloomery_workspace::source`; their existing
`bloomery_scanning` re-exports remain available. Call `scan_tree(&tree,
&context.config.scanners)` and pass its evidence to
`bloomery_check::validate_evidence(&context, &analysis.output.evidence)` for
static traceability validation. Scanner completeness describes syntactic
discovery, while relational diagnostics describe references and coverage.
Neither API runs tests or grants hosted approval.

Compatibility: manually constructed `Context` values must supply resolved
`specs_enabled`. Filesystem loading follows the shared symlink/ignored-directory
rules, and unreadable referenced design files fail even without an anchor.
The [versioned report API](../check/README.md) composes loading and discovery
with canonical identities and explicit completeness. `MemoryTree::from_bytes`
preserves binary content for snapshots; text constructors remain available.
Target/feature resolution remains outside syntactic analysis.

`bloomery_workspace::cargo::load_test_target(tree, manifest, name)` projects a
unique explicit `[[test]]` name/path, required features and harness mode using
the same tree adapter and existing TOML parser. It normalizes manifest-relative
entrypoints within the repository and rejects missing files or duplicate names.
`cargo::validate_selector` checks selector paths/names without reading content.
This helper performs no Cargo invocation, automatic target discovery, feature
resolution or hosted specification discovery.

`source::resolve_relative_path(directory, path)` performs shared lexical path
normalization inside the repository. Cargo entrypoints and module projection use
this policy; it does not read content or follow symlinks.
