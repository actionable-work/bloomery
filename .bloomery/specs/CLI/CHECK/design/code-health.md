# Code health

## Check integration

`static:code-health` measures first-party Rust source, calculates crate and
workspace scores, and checks configured limits against the existing baseline.
It is opt-in through `checks.codeHealth.enable`, which defaults to `false`, independently of
`specs.enable`, `scanners.rust.enabled`, and the Nix generation toggle
`checks.enable`. It appears once in the catalog, regardless of selected systems.
Normal selectors, formatting, scheduling, cancellation, failure retention, and
exit codes apply. Disabled health contributes no catalog entry or analysis.

Health has no specification prerequisite. In write mode it runs after the
formatter and beside structure and Nix work; a specification failure cannot
suppress it in default mode. In no-write mode it runs alongside the batched
native treefmt and selected Nix checks. Traceability and health share the command-scoped Rust analysis described
in [shared analysis](../../../PARSER/SCANNING/design/health-analysis.md).
Each consumer receives only its requested projections and diagnostics. The
shared service publishes completed file facts without requiring simultaneous
task slots, including with `--jobs 1`.

Inline Clippy suppression policy is a separate static check described in
[Clippy suppressions](clippy-suppressions.md). It does not affect health scores.

The health index summarizes source maintainability and crate modularity.
Production SLOC per crate is a proxy for small compilation units; it does not
measure build times. Existing Clippy checks remain independent gates. Coverage,
compiler diagnostics, unsafe frequency, binary size, and dependency cycles
contribute no score: the index has no compiler, runtime, or artifact inputs and
makes no safety or correctness claim.

## Metrics and score

Metric semantics are owned by the
[Rust analysis contract](../../../PARSER/SCANNING/design/health-analysis.md#metrics).
Every crate reports production SLOC, callable count, per-callable cyclomatic
complexity, maximum nesting, body token count, and exact duplicate token coverage.
Report maxima, counts above configured soft limits, and the contributing source
locations. Empty bodies still constitute callables.

For metric value `x` and its positive soft limit `t`, define
`excess(x,t) = min(1, max(0, (x-t)/t))`. For `F` callables and duplicate fraction
`d`, plus crate production SLOC `s`, the five normalized penalty rates are:

```text
complexityRate = mean(excess(cyclomatic, cyclomaticLimit))
sizeRate       = mean(excess(body_tokens, functionTokensLimit))
nestingRate    = mean(excess(nesting, nestingLimit))
duplicateRate  = min(1, max(0, (d-duplicationFreeRatio)/(1-duplicationFreeRatio)))
crateSizeRate  = excess(s, crateSlocLimit)
```

The effective relative weights are `complexity`, `size`, `nesting`,
`duplication`, and `crateSize`, defaulting to 25, 20, 10, 15, and 30. For their
sum `W`, calculate each deduction as `100 × weight / W × penaltyRate`; the score
is the greater of zero and 100 minus the five deductions. Weight values are
finite numbers from 0 through 100 with at most two decimal places. Any component
may have weight zero, but the effective weights must have a positive sum.
Omitted component keys retain their defaults, so a partial table is valid.
These relative weights apply uniformly to every selected crate; the workspace
score and subscores remain the production-SLOC-weighted means of applicable
crate results. A crate above `crateSlocLimit` incurs a crate-size deduction,
which makes splitting a large crate into smaller crates improve the workspace
score. Score arithmetic uses the exact weight ratios and rounds persisted scores
once to integer hundredths, nearest with ties upward.

An empty callable population has zero mean penalties. A crate with zero
production SLOC has score `null`, status `not_applicable`, and zero aggregate
weight. A nonempty crate with no callables can score 100, with its zero callable
population visible. The workspace score is the SLOC-weighted mean of applicable
crate scores. If total production SLOC is zero, the workspace score is `null`
and health passes as `not_applicable` without requiring a baseline. There is
no separate cross-crate penalty: cross-crate duplicates already contribute to
each affected crate's duplicate fraction.

Use deterministic arithmetic in algorithm version 2 and snapshot schema version
4. Persist score values
in integer hundredths of a point, round once using nearest with ties upward,
and compare those values for all gates and deltas. Workspace weighting uses
the persisted crate values. Display two decimal places. Persist raw metrics,
soft limits, effective weights, and each weighted penalty so a score can be
explained and reproduced. Thresholds and relative weights are configurable.

## Policy gates

An applicable crate fails below `minCrateScore`; the applicable workspace fails
below `minWorkspaceScore`. Any crate above its effective `maxCrateSloc` and any
callable above its effective `maxCyclomatic`, `maxFunctionTokens`, or
`maxNesting` fails even when its score passes. A crate exception overrides only
the hard limits it names for the exact workspace-relative `Cargo.toml` path;
omitted limits and all other crates use the global values. An exception path
that does not identify a selected workspace crate is a configuration error.
Equality passes. Record each exceeded limit with its measured value, effective
limit, crate identity, and source span when applicable.

Compare each matching applicable crate and the workspace against the baseline.
A decrease greater than `maxRegression` points fails; improvements and equality
pass. The workspace comparison uses each report's own SLOC weights. Added crates
have no prior score and must pass absolute gates; removed crates are reported
as removed. Changes to applicability have nullable deltas. Crate identity is
the repository-relative manifest path, with package name as a display field;
renames at the same path retain identity.

Malformed, unsupported, or policy-incompatible baselines fail health with
actionable diagnostics after measurement; they never erase a valid current
report. A missing baseline fails in no-write mode only for applicable health;
complete write mode initializes it without a failure. Compatibility requires
matching snapshot schema, algorithm version, effective scoring limits and
weights, duplicate window size, and source-selection policy (manifest location
and configured member selection). Absolute score floors,
hard limits, enablement, baseline path, and allowed regression do not change
score comparability. Membership and source changes within the same selection
policy are compared normally.

## Retained reports

Every completed measurement retains a report in the ordinary run store,
including when baseline or policy gates fail. The report records schema and
algorithm versions, effective policy, a deterministic input digest, crate
identities and source inventory, raw metrics and penalty breakdowns, current
and baseline scores, deltas, added/removed crates, and policy diagnostics.
The SHA-256 input digest covers manifest discovery inputs, selected source bytes and
paths, and effective health configuration, including crate hard-limit exceptions.
Run metadata holds revision and
working-tree state; the portable snapshot contains no timestamp, absolute
path, run ID, or revision identifier. Unavailable or interrupted measurement
cannot yield a complete report or an exportable baseline.

`bloomery check health [--run RUN] [--offset N] [--limit N] [--json]` reads a
retained report without analysis, formatting, or builds. Default run selection
is the latest completed run; absent health data is an explicit retrieval error
without fallback. Page crate summaries in manifest-path order with offset 0,
limit 20, and an 8 KiB whole-response ceiling. Return workspace score, baseline
score, delta, applicability, actual offset, total, and nullable next offset.
Use the normal bounded-field and continuation rules. Failure locations and
metric contributions remain available through `check details`; full raw
metrics remain in the retained run report. Successful retrieval exits 0 even
if the retained check failed; invalid or unavailable requests exit 2.

The bounded check summary supplies only the run-specific health retrieval
command when a report was retained. It does not announce automatic baseline
refresh or print the baseline path. The command identifies the run, so do not
repeat the health notice or run ID elsewhere in the summary. Health failures
participate in ordinary failure pagination and details; no per-crate success
lines are added to normal output.

## Baseline snapshots

`bloomery check health update [--run RUN] [--json]` atomically replaces the file
at `checks.codeHealth.baseline` with the selected run's complete current
measurement. A normal write-mode check also replaces this file after
`static:code-health` completes a measurement. It first compares against the
existing baseline, then writes the new measurement even when health policy gates
or unrelated checks fail. If the baseline is missing, a complete write-mode
check initializes it from the current measurement without a missing-baseline
failure. Absolute score and hard-limit gates still apply, and the retained report
records `baseline_status = initialized`. With `--no-write`, an absent baseline
fails with an instruction to rerun `bloomery check` in write mode or publish a
retained measurement with `bloomery check health update`. If an existing baseline
is invalid or incompatible, the check retains the corresponding failure and seeds
the file from the complete measurement. This makes the file the latest comparison
point for subsequent checks. Regression limits therefore compare each selected
write-mode run with the baseline that existed when that run began.
If atomic replacement fails, report `HealthBaselineWriteFailed`, fail the
health check with exit code 1, retain the complete measurement, and preserve
the previous baseline.

Automatic refresh occurs only when code health is enabled and selected. It does
not occur after a failed write-mode formatter preflight, an incomplete health
measurement, or cancellation or interruption before measurement completes.
With `--no-write`, health still compares against the existing baseline but
never replaces it. A complete no-write measurement can still be explicitly
published later with `check health update`.
`check health update --no-write` is a usage error before Bloomery reads the
retained measurement or modifies the baseline.

The explicit update command remains available for publishing a retained
measurement, including one produced by no-write execution. A typical sequence
is:

```text
bloomery check --check static:code-health
bloomery check health --run RUN
bloomery check health update --run RUN
```

Update can export a complete measurement whose policy gates failed, including
a no-write measurement with a missing-baseline failure. It requires passed
formatting outcomes
for the run: the `format:workspace` preflight in write mode or the native
`core:treefmt-check` for every selected system in no-write mode. It also
requires a finalized, uninterrupted run with complete health measurement;
unrelated check failures do not prevent export. Verify the current input digest against
the retained digest using source reads and discovery, without AST parsing,
formatting, or builds. Reuse the retained module graph for source paths;
changed graph-defining source or manifest bytes already invalidate its digest.
Changed sources, manifests, inventory, or effective health configuration
reject export as stale. Do not copy baseline-relative fields or gate diagnostics
into the snapshot: it stores current measurements and their provenance, suitable
for the next comparison.

The checked-in `.bloomery/code-health-summary.json` is both the PR summary and
the comparison baseline. It uses schema version 4 and has top-level fields
`schema_version`, `algorithm_version`, `policy`, `input_digest`, `workspace`,
and `crates`. The workspace and each crate row expose `overall_score`,
production `sloc`, and category `subscores` for complexity, function size,
nesting, duplication, and crate size. Subscores are deduction points in each
category.
Crate rows also include the display name and repository-relative `manifest_path`
used as their stable identity. Workspace score and subscores are SLOC weighted
from crate results. The policy retains effective scoring limits, weights, and
source-selection inputs; the digest records which inputs produced the snapshot.

The file omits callable metrics and locations, duplicate ranges, source
inventories, opaque macro locations, and run metadata. Those remain in retained
run reports and `check details` for deeper investigation. Loaders reject
duplicate crate identities, invalid score/SLOC combinations, inconsistent
workspace aggregates, unsupported versions, invalid digests, and incompatible
scoring or selection policies.

The summary uses stable field and crate order, two-space indentation, and a
trailing newline. Re-exporting identical measurements produces identical
bytes. Replace the configured destination through a temporary file and atomic
rename. The path must remain inside the repository, including through
symlinks, and cannot overlap source inputs, manifests, configuration, locks, or
specifications. Report the written path and crate/workspace scores in bounded
human or JSON output for automatic refresh and explicit update. Invalid
requests exit 2; stale inputs or write failures exit 1.

Retrieval never modifies the baseline. Git or jujutsu history supplies prior
snapshots; Bloomery does not create commits or maintain a second repository
history store.
