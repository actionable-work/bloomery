# Direct test-entrypoint modules

`scan_tree_with_rust_root(tree, scanners, root)` retains configured syntactic
discovery and additionally projects direct out-of-line module declarations from
one explicit Rust root. The root can be outside scanner globs; its declarations
provide context without adding unselected test sites. Rust disablement prevents
root reads. The shared AnalysisSession parses each source once.

Owned RustModule facts retain declaration names, identifier spans, candidate
paths, inherited root/declaration cfg syntax and unsupported-path state. Plain
`mod child;` proposes sibling `child.rs` and `child/mod.rs`; literal `#[path]`
proposes its authored path. Candidate construction is shared with filesystem
health analysis. Virtual projection never probes the filesystem.

The report resolves candidates against the authorized tree. Paths normalize
within the repository. Exactly one existing candidate establishes direct
membership. Missing, ambiguous, escaping or self-referential paths and malformed,
duplicate or conditional path attributes stay unresolved and produce located
static findings. Rejected cfg gates suppress irrelevant path findings.

Target conditions combine required-feature eligibility, entrypoint file gates
and declaration gates. Multiple declarations reaching one file combine by any
satisfied route. Local file/module/test cfg remains a separate condition.
Only configured test bindings in the entrypoint or known direct children can
credit coverage. Other files remain unresolved. Literal findings remain visible.

`CargoTestRecord.modules` exposes canonical candidates, resolved paths,
conditions and declaration spans. `completeness.rust_modules` describes this
direct projection; partial, failed, disabled and unattempted states are explicit.
Nested/inline module membership, conditional paths, macro expansion and complete
compilation remain unsupported. Relations stay partial. No test execution or
hosted approval occurs. Report/parser contracts are 6; spec schema remains 1.
