# Source spans

`SourceLocation` retains an optional start line/column and optional end
line/column. Coordinates are one-based Unicode scalar positions. End positions
are exclusive. Tabs count as one scalar; LF advances the line, including in
CRLF input. Unknown columns and ends remain absent.

Rust, Playwright and Nix references identify the complete quoted literal,
including delimiters and a Playwright tag prefix. Rust test sites identify the
function name; Playwright sites identify the complete call; Nix file sites
identify the whole file, including an empty file's zero-length range.

Offsets come from existing parser or tokenizer spans and the exact source
snapshot. Mapping UTF-8 byte offsets to coordinates does not read files again
or parse source again. Multiline literals retain multiline ranges. Source
diagnostics retain precise ranges when available: malformed Rust tags, labeled
TypeScript parse errors and invalid Nix metadata strings. Read failures and
Rust ingestion errors without retained parser spans keep their existing
line-only locations rather than fabricating ranges.
Diagnostic ordering includes the complete location, code, message and notes so
same-line findings remain deterministic regardless of evidence input order.

Shared relational validation preserves the reference location in orphan and
manual-automation findings. Static reports preserve complete locations for
bindings, tests and diagnostics. ID trace and candidate projections retain
their existing fields. Span precision does not change discovery scope,
completeness, reference resolution or the static execution boundary.
