# Static repository inputs

Specification discovery reads only repository tree content. The same directory
hierarchy, Markdown frontmatter, TOML requirements and design links define the
effective specification set for local and virtual repositories. Hosted systems
manage specification lifecycle by changing repository files; discovery has no
hosted registry, lifecycle service, generated catalogue or network fallback.

## Shared tree boundary

`bloomery_workspace::source` owns `SourceTree`, `MemoryTree`, `FilesystemTree`
and the validated `TreeIndex`. Scanning re-exports the same public tree types.
The index retains canonical relative file and directory paths, infers ancestors
and represents explicit empty directories. Duplicate paths and file/directory
collisions are errors. Filesystem traversal excludes symbolic links and standard
ignored directories. A selected file whose content is unavailable is an error.
`SourceTree::read_bytes` preserves raw file content for snapshot identity;
`read` supplies UTF-8 documents to parsers. Text-only adapters inherit byte
reads from their text implementation. `MemoryTree::from_bytes` preserves binary
files and explicit directories without interpreting unselected content.

## Loading

`load_tree(tree, config)` accepts a caller-authorized tree and explicit validated
`Config`. It returns a `Context` with repository-relative locations and an empty
root path. It neither reads a configuration file nor accesses the filesystem.
The filesystem `load(root)` reads configuration and delegates to the same loader,
then roots domain paths and diagnostic locations at the supplied repository root.
Diagnostic messages and notes use relative paths for adapter parity.

`specs.dir` resolves exactly one active root below `.bloomery/`. Absent enablement
auto-detects that root; explicit enablement requires it. Explicit disablement
returns an empty context without enumerating or reading the tree. Legacy paths
are not implicitly merged. IDs retain their file spelling and existing duplicate
validation applies across the active tree.

The loader shares hierarchy, requirement and design-anchor validation across
adapters. Document content is cached for one load, including read failures;
unavailable design content cannot silently validate a link without an anchor.
Contexts and diagnostics have deterministic ordering independent of index order
or filesystem root. Configured empty design directories remain valid.

## Static relations

The public relational validator accepts a resolved context and discovered
evidence. Filesystem checking and virtual callers use the same orphan-reference,
illegal-manual-automation and missing-automated-test rules. It sorts diagnostics
and performs no test execution or hosted approval. Source discovery completeness
remains distinct from relational coverage and test success.
