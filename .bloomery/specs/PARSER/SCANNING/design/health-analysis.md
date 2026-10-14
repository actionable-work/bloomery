# Shared Rust health analysis

## Source selection

Health selects the workspace manifest at `build.cargoToml` and the member
selection at `build.members`, using the same pure workspace discovery and
manifest target rules as the Nix build. Reuse that discovery projection in
catalog setup where possible; do not run Cargo metadata or introduce a second
manifest resolver. External dependencies and configured sub-flakes are outside
this workspace's health scope. Package identity is its repository-relative
manifest path; group library, binary, and proc-macro targets under that identity.

Start from production target entry points. Resolve inline and file modules,
literal `#[path]` alternatives, and literal Rust `include!` paths relative to
their source file. Inspect all source-level feature and platform alternatives
without evaluating `cfg`, except for the test exclusions below. Canonicalize
and deduplicate files so shared modules used by multiple targets count once
within a crate. A source shared by different crates contributes to each owner,
but is read and parsed once per command and does not duplicate itself in clone
detection. Paths outside the repository and unresolved required literal modules
produce source diagnostics rather than silently shrinking coverage.

Exclude test, example, benchmark, and build-script targets; exclude functions
with test attributes and subtrees explicitly guarded by `#[cfg(test)]`.
Other conditional expressions are conservatively included. This is a lexical
source model, not a measurement of one compiled feature set. Generated files
and macro expansions are not analyzed. Nonliteral includes and item macros
remain opaque and are listed by location in the report; source tokens still
contribute to production SLOC. Macro invocations do not manufacture callables
or hidden control flow. The report identifies these boundaries.

Health selection is independent of evidence globs. Scanner settings cannot
hide production code from health, and health exclusions cannot hide evidence
from traceability.

## Shared pass

Build a command-scoped union of requested Rust sources. The shared ingestion
service owns reads, canonical paths, source text, line indexes, `syn` parsing,
and compact derived facts. A file is read and parsed at most once, including
when health and traceability start at different times or module discovery
adds it later. Cache failures as well as successes. Provide synchronized
per-file initialization without holding a scheduler slot while waiting for a
different check task to start. Discard all cached state when the command ends.

One traversal derives requested requirement references, test sites, callable
metrics, and production token streams from each tree. Reuse attribute
classification, source spans, and module discovery facts across projections.
Discard trees after deriving reusable facts to bound memory by source facts
and active file parses. A later consumer uses cached facts, never reparses.
Clone matching is a workspace-wide pass over derived token windows; it neither
reads source nor builds another syntax tree.

Read/parse failures are retained once per file, with affected consumer check
IDs. They fail affected consumers independently: evidence-only source errors
do not invalidate health, health-only errors do not invalidate evidence, and
malformed evidence tags do not invalidate health metrics. A health source error
prevents publication of a complete score or baseline for the workspace.
Unused projections perform no analysis; health alone never scans Playwright,
Nix evidence, or specifications. No analysis launches Cargo, rustc, Clippy,
test binaries, external analyzers, or network requests.

## Metrics

A callable is a free function, method, default trait method with a body, or
closure. Declarations without bodies are not callables. Nested callable bodies
belong to their own metric unit: their control flow and body tokens are excluded
from the enclosing unit. Identity combines manifest path, source path, lexical
item path, and occurrence ordinal; spans use actual token positions, not string
search for a function name.

Cyclomatic complexity starts at 1 and adds one per `if`, `for`, `while`, `loop`,
`&&`, `||`, and `?`; each match adds `max(0, arm_count-1)` plus one per guarded
arm. Count explicit arms, including wildcard arms. An `else if` is another
`if`; unconditional `return`, `break`, and `continue` add nothing. Count only
visible AST expressions, excluding macro bodies and nested callable bodies.

Nesting starts at zero and records the maximum depth of nested `if`, match,
and loop bodies. An `else if` stays at its sibling `if` depth; arbitrary blocks
do not increase depth. Closures restart depth as separate callables.

Body size counts lexical Rust tokens inside a callable's braces or closure
expression. Identifiers, keywords, literals, and punctuation each count as one;
opening and closing delimiters each count as one. Comments and whitespace do
not count. Macro argument tokens count toward size, without interpreting them
as Rust expressions. Reuse tokens from the parsed input; do not run a second
source lexer. Production SLOC counts distinct physical lines containing at
least one noncomment production token, including declarations, attributes,
and delimiters. Multiline tokens count their occupied lines. Test-excluded
tokens do not contribute; shared target files count once per crate.

Exact duplication compares contiguous windows of `duplicateMinTokens` body
tokens across distinct callables, retaining token kinds, identifier spelling,
and literal spelling while ignoring comments, whitespace, and source spans.
Do not normalize names or literals. Windows cannot cross callable boundaries.
Hash windows for candidate grouping and compare complete token sequences to
confirm equality; hash collisions cannot create duplicates. Merge overlapping
matched ranges. A physical range compared to itself through another target or
crate ownership is not a clone; disjoint callables in the same file can be.

Duplicate fraction is the union of covered duplicate body-token positions
divided by all production callable body tokens in that crate, or zero when
that denominator is zero. Cross-crate matches mark covered positions in each
affected crate. Repeated matches cannot push the fraction above one. Retain
paired source ranges and owning crate identities for explanations.

The scoring and persistence contract lives in
[CLI/CHECK](../../../CLI/CHECK/design/code-health.md).
