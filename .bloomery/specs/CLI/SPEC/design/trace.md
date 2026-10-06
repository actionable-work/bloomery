# Traceability projections

`bloomery spec trace` and `bloomery spec candidates` are read-only projections
over the resolved requirement model and the statically discovered test sites.
Both require the shared root `flake.nix` and `.bloomery/config.toml`
preflights. Neither mutates the specification tree, configuration, or lockfiles,
and neither executes tests.

```text
declared requirements ──┐
                        ├──► trace:    requirement → tied tests
discovered test sites ──┘   candidates: test sites → no requirement
```

## Trace

`bloomery spec trace ID...` accepts one or more exact requirement IDs. For each
ID it reports the record identity and every statically discovered test
reference tied to it: scanner kind (`rust`, `playwright`, or `nix`), repository-
relative source path, and line. A requirement with no tied tests is reported
with an empty reference list and succeeds; the missing evidence itself is a
[check](../../CHECK/README.md) error, not a trace error. An ID absent from the
declared model is a usage error naming the ID.

Reference ordering is deterministic by scanner, path, and line. Trace uses the
same `scanners.*` configuration and static boundary as
[static evidence scanning](../../../PARSER/SCANNING/README.md).

## Candidates

`bloomery spec candidates` lists discovered test sites that carry no
requirement reference. Each candidate reports the scanner kind, the test name
when available, the repository-relative path, and the line. Ordering is
deterministic by scanner, path, and line.

Candidates are advisory graduation input. The command does not create
requirements, does not assign identifiers, and does not fail because untied
tests exist. Tests referencing an unknown or manual requirement are not
candidates; those relationships are errors owned by check.

## Discovery boundary

Test sites are discovered only where the configured scanner can identify a test
declaration statically. Rust test functions and Playwright `test(...)` calls
are discoverable whether or not they carry a requirement tag. Nix test files are
selected by `scanners.nix.testPaths`, which defaults to `["**/*.test.nix"]`: a
matched file with `passthru.bloomery` metadata is tied, and one without metadata
is an untied candidate. Discovery does not evaluate Nix, so a Nix check outside
the configured test paths is not a candidate. See
[test discovery](../../../PARSER/SCANNING/design/test-discovery.md).

## Output

Human output prints a heading per requirement or a flat candidate list,
highlighting IDs, scanner kinds, and source locations. `--json` emits one
structured document per invocation: trace emits a record per requested
requirement with its tied test references, and candidates emits an array of
test-site records. JSON is deterministic and contains no ANSI escapes.