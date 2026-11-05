# Source trees

The public `bloomery_scanning` library accepts a `SourceTree` and explicit
`ScannersConfig`. `MemoryTree` owns immutable UTF-8 content keyed by canonical
repository-relative paths. `FilesystemTree` adapts a repository directory.
These tree types and their validated index are owned by `bloomery_workspace::source`
and re-exported by scanning.
Both use the same glob selection, parsers, reference extraction and test discovery.
No temporary checkout, configuration file, subprocess or network is required by
the memory adapter.

## Inputs

Paths use `/` separators and contain only nonempty normal components. Absolute,
parent, dot, duplicate and file/directory-colliding paths are rejected. A listed
file whose content cannot be read produces a diagnostic, never an empty source.
Filesystem traversal uses the scanner's standard ignored directory names and
does not follow symbolic links. Scanner configuration is validated before reads.
Disabled scanners do not read content.

## Results

`scan_tree` returns repository-relative references and test sites, plus one
explicit state per supported scanner: `disabled`, `no_matching_files` or
`complete`. A failed read or parse returns diagnostics and no successful result.
An analysis is complete only when every scanner is complete. Completeness means
syntactic discovery within the configured globs; it does not mean specification
coverage, target/feature resolution, or test success. Rust cfg conditions are not
evaluated, Playwright calls are inspected syntactically, and Nix metadata is
tokenized without evaluation. Unresolved requirement IDs remain literal bindings
for the relational checker to validate.

References are ordered by scanner, path, line, column and requirement ID. Test
sites are ordered by scanner, path, line, column and name; their requirement IDs
are sorted and deduplicated. Diagnostics use the shared diagnostic ordering.
Identical content and configuration yield identical results regardless of tree
insertion order or filesystem root. The filesystem `scan_all` compatibility API
retains source locations rooted at `Context.root` and shares Rust ingestion with health checks.
The existing CLI trace and candidate commands consume that adapter.

This API emits discovery facts only. Execution results and approval authority
are outside its result model.
