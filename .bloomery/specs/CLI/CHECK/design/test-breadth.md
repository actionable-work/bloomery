# Test requirement breadth

`static:traceability` fails with `TooManyRequirementsForTest` when one authored
validating site names more distinct declared automated requirements than
`checks.specifications.maxRequirementsPerTest`. The default and upgrade
recommendation are `5`; equality passes. Specification management and
`checks.specifications.enable` gate this policy with the other relational rules.

## Counting

Use the site identities in [specification granularity](spec-granularity.md#counting):
one Rust test function, one directly tagged Playwright call, or one Nix evidence
file. Deduplicate IDs and repeated discovery of the same site. Different sites
are assessed separately, including declarations with the same name. Nix file
counts include all literal metadata associations, independently of candidate
test globs. Suite inheritance, runtime parameter rows, retries, macro expansion,
and selected systems do not multiply associations.

Only declared automated IDs consume the limit. Orphan and manual references
retain their independent diagnostics. Rust cfg, target and ignored-test
eligibility do not erase authored associations. Observed excess can establish
a violation during partial discovery; unavailable discovery cannot certify
compliance. This is a coupling heuristic, not execution coverage.

## Configuration and exceptions

The global limit is a positive integer. `testExceptions` maps nonempty labels
to tables with required repository-relative `path` glob, positive
`maxRequirements`, nonempty `reason`, and optional exact test `name`.
Name-specific matches precede path-only matches; overlapping matches within
each category use lexicographic label order. Names are literal, not globs.
Unmatched exceptions do not hide facts or scanner failures. Policy shape and
types validate before formatting, including when checks are disabled.

Keep integration exceptions scoped to exact paths and test names. File-level
Nix suite aggregation can use a path-only exception because its static unit
contains several evaluated checks. All exceptions remain visible with their
effective limit and reason.

```toml
[checks.specifications]
enable = true
maxRequirementsPerTest = 5

[checks.specifications.testExceptions.protocol_roundtrip]
path = "tests/protocol.rs"
name = "roundtrip_contract"
maxRequirements = 7
reason = "One protocol roundtrip checks its cross-layer contract."
```

## Findings and reports

Emit one finding per over-limit site at its source location. Include the test
name when available, observed count, effective limit, and every contributing
ID with its declaration location in ID order. Retain the exception reason when
one applies. Recommend separating independent test stories; do not rewrite tests
or remove traceability annotations automatically. Normal failure paging,
retained details, and exit code 1 apply.

`quality.tests` lists sites with at least one declared automated association,
their names, sorted distinct requirement IDs, effective limits, and exception
reasons in scanner/path/line/column order. Human full reports show the inventory
count and explicit overrides. Filesystem, virtual, caller-supplied and scoped
analysis use the same associations. Parser/report contract 10 binds this
projection and validation rule; grouped specification schema remains 1.
