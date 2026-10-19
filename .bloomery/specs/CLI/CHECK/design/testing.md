# Hermetic Nix boundary tests

## Default suite

The default `nix run .#bloomery:dev -- check` executes every CLI/CHECK automated
scenario through ordinary host-system check outputs. The Rust test derivation
and a catalog-expression check are selected by the default suite. Their
failures fail the run through existing Nix outcome handling. No additional
command, opt-in test flag, host hook, or runner check category is required.

Every scenario executes its assertions unconditionally. Tests do not probe for
Nix, a daemon, network access, or a writable store to decide whether to run.
Unavailable fixture executables or fixture data fail the test. Ignored tests,
early successful returns, and assertions reached only on developer hosts do
not supply requirement coverage.

## Test boundaries

| Boundary | Test mechanism | Assertions |
| --- | --- | --- |
| Runner | In-memory `NixBackend` | Selection, scheduling, mode-specific formatting, retained outcomes and failures |
| Production `NixCli` adapter | Strict local subprocess fixture | Arguments, working directory, stdin, JSON decoding, exit handling, captured output, call order |
| Catalog expressions | Nix evaluation assertions exposed as a flake check | Actual production projections over fixture check sets, alias identity, attribute spelling and per-attribute evaluation failures |

The subprocess fixture is a small repository-owned executable supplied by the
test closure and selected through explicit executable injection. It implements
scripted command responses, records invocations, and rejects unexpected calls,
arguments, or stdin. Each scenario asserts that all expected calls occurred.
Executable injection covers both `nix` and any `nix-store` helper calls so no
adapter path falls through to an ambient host tool.
It does not interpret Nix expressions, invoke Nix, or contact a store. Tests use
private fixture directories without changing the process-wide `PATH` or
depending on `/bin/sh`. Fixture tools and source files are declared inputs;
missing inputs are failures. Existing in-memory backend tests remain useful
for runner policy but do not replace production adapter tests.

Catalog projection expressions have one source shared by production adapter
invocations and Nix evaluation tests. Co-located `.nix` sources are embedded in
the Rust adapter so the installed CLI needs no source checkout. Nix tests
import those sources and evaluate fixture check sets during the outer flake
evaluation. Evaluated expectation results feed an ordinary check derivation;
any failed expectation makes that derivation fail. An expression evaluation
error fails the run during discovery. The builder does not start a Nix client.
Tests force all expected results so lazy values cannot conceal failures.
Projection fixtures use genuine derivation values without realizing their
builders. Pure expected values assert the JSON contract, including successful
and failed plan entries.

## Behavioral coverage

| Scenario | Required assertions | Requirements |
| --- | --- | --- |
| Aliased checks | Nix projection preserves both names with equal derivation paths; adapter submits one batch with each derivation installable once; runner retains an outcome for each alias | `CLI-CHECK-NIX-007`, `008`, `009`, `019`, `020` |
| Unusual names | Nix projection and adapter decoding preserve spaces, quotes, Unicode, newlines, tabs, control characters, percent signs and fragments; realization arguments contain only derivation installables | `CLI-CHECK-NIX-006`, `016` |
| Missing system | Adapter handles an unsuccessful catalog command as an operational error and runner rejects the explicit selection | `CLI-CHECK-SELECT-011` |
| Empty system | Adapter decodes a successful empty catalog and runner accepts it when other selected work exists | `CLI-CHECK-SELECT-020` |
| Mixed batch results | Production adapter processes a failed batch with valid sibling outputs; runner retains individual pass/fail outcomes | `CLI-CHECK-NIX-021`, `023`, `027` |
| Causal logs | Adapter processes dependency metadata and failure output; runner retains the causal derivation and retrieves its log through the production adapter | `CLI-CHECK-NIX-024`, `025`, `026` |
| Write-mode formatter success | Production adapter invokes formatting once; a fixture edit is visible to subsequent checks and remains in the workspace | `CLI-CHECK-FORMAT-001`, `003`, `004` |
| Write-mode formatter failure | Nonzero formatter exit retains its output and edits, fails the run, and admits no selected check work | `CLI-CHECK-FORMAT-002`, `004`, `006`, `007` |
| No-write native formatter check | The runner adds `nix:<system>:core:treefmt-check` for selected systems even when partial selectors omit it, batches it with Nix checks, retains its native outcome and derivation log, changes no workspace file, and deduplicates explicit selection | `CLI-CHECK-FORMAT-008`, `010`, `011` |
| Missing native formatter check | Missing `core:treefmt-check` on a selected system is an operational catalog error before check work starts | `CLI-CHECK-FORMAT-012` |
| No-write formatter failure | A failed formatting task is an ordinary check failure; independent work continues unless fail-fast stops or cancels it | `CLI-CHECK-FORMAT-009` |
| No-write command policy | Accept the global option; reject writers early; allow read operations, preview modes, help/version, run cache and Nix store outputs | `CLI-INTERFACE-FLAGS-032`–`036`, `CLI-INIT-COMMAND-013`, `CLI-SYNC-INTERFACE-024`, `CLI-SPEC-EDITING-024`, `CLI-CONFIG-EDITING-021`, `CLI-CONFIG-UPGRADE-027`–`028`, `CLI-CONFIG-DOCUMENT-013`, `CLI-CHECK-HEALTH-038`, `CLI-BUILD-EXECUTION-020`, `028` |
| Health baseline refresh | Write-mode selected health compares then refreshes after a complete measurement, including after policy or unrelated failures; missing or invalid baselines are seeded | `CLI-CHECK-HEALTH-032`, `033`, `036` |
| Health baseline write failure | A failed atomic replacement reports `HealthBaselineWriteFailed`, retains the report, and leaves the previous file intact | `CLI-CHECK-HEALTH-028`, `039` |
| Health baseline no-write and skips | No-write compares without replacing; disabled or unselected health, incomplete measurements, failed formatting, and interrupted runs preserve the file | `CLI-CHECK-HEALTH-034`, `035`, `037` |
| Missing executable | Adapter startup failure yields an operational error and cannot produce a successful run | `CLI-CHECK-NIX-012` |

Batch tests assert `--keep-going`, `--no-link`, lockfile protection, safe
argument boundaries, output-validity stdin and log retrieval. Fail-fast tests
assert cancellation and unstarted outcomes. Catalog tests include an attribute
that fails evaluation alongside valid siblings and the excluded legacy
recursive output. Write-mode call order discovers check IDs before formatting
and resolves derivation paths only after formatting succeeds. No-write resolves
IDs and paths once per system, then selects the native treefmt derivation into
the batched Nix realization, including when the user selects only static checks.

Alias tests verify Bloomery's responsibility to submit a shared derivation once
and retain each alias outcome. Nix owns store locking and physical realization.
No fixture emulates or claims to verify Nix's internal locking, builder
sandbox, daemon, or substitution implementation.

## Requirement evidence

Keep behavior requirements automated with their existing IDs and semantics.
Rust assertions carry the IDs for runner and adapter behavior. Nix checks carry
literal `passthru.bloomery` IDs for evaluated projection assertions. Attach each
ID to the test that exercises that behavior; fixture declarations alone are
not behavioral evidence. The default suite executes these evidence-bearing
tests, including the formatter and causal-log scenarios.
