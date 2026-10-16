# Code health

## Check integration

`static:code-health` measures first-party Rust source, calculates crate and
workspace scores, and checks configured limits and a reviewed baseline. It is
opt-in through `checks.codeHealth.enable`, which defaults to `false`,
independently of
`specs.enable`, `scanners.rust.enabled`, and the Nix generation toggle
`checks.enable`. It appears once in the catalog, regardless of selected systems.
Normal selectors, formatting, scheduling, cancellation, failure retention, and
exit codes apply. Disabled health contributes no catalog entry or analysis.

Health has no specification prerequisite. After formatting, it can run beside
structure and Nix work; a specification failure cannot suppress it in default
mode. Traceability and health share the command-scoped Rust analysis described
in [shared analysis](../../../PARSER/SCANNING/design/health-analysis.md).
Each consumer receives only its requested projections and diagnostics. The
shared service publishes completed file facts without requiring simultaneous
task slots, including with `--jobs 1`.

Inline Clippy suppression policy is a separate static check described in
[Clippy suppressions](clippy-suppressions.md). It does not affect health scores.

The health index summarizes source maintainability. Existing Clippy checks
remain independent gates. Coverage, compiler diagnostics, unsafe frequency,
binary size, and dependency cycles contribute no score: the index has no
compiler, runtime, or artifact inputs and makes no safety or correctness claim.

## Metrics and score

Metric semantics are owned by the
[Rust analysis contract](../../../PARSER/SCANNING/design/health-analysis.md#metrics).
Every crate reports production SLOC, callable count, per-callable cyclomatic
complexity, maximum nesting, body token count, and exact duplicate token coverage.
Report maxima, counts above configured soft limits, and the contributing source
locations. Empty bodies still constitute callables.

For metric value `x` and its positive soft limit `t`, define
`excess(x,t) = min(1, max(0, (x-t)/t))`. For `F` callables and duplicate fraction
`d`, the four penalties are:

```text
complexity = 40 × mean(excess(cyclomatic, cyclomaticLimit))
size       = 25 × mean(excess(body_tokens, functionTokensLimit))
nesting    = 10 × mean(excess(nesting, nestingLimit))
duplicates = 25 × min(1, max(0, (d-duplicationFreeRatio)/(1-duplicationFreeRatio)))
score      = max(0, 100-complexity-size-nesting-duplicates)
```

An empty callable population has zero mean penalties. A crate with zero
production SLOC has score `null`, status `not_applicable`, and zero aggregate
weight. A nonempty crate with no callables can score 100, with its zero callable
population visible. The workspace score is the SLOC-weighted mean of applicable
crate scores. If total production SLOC is zero, the workspace score is `null`
and health passes as `not_applicable` without requiring a baseline. There is
no separate cross-crate penalty: cross-crate duplicates already contribute to
each affected crate's duplicate fraction.

Use deterministic arithmetic in algorithm version 1 and snapshot schema version
2. Persist score values
in integer hundredths of a point, round once using nearest with ties upward,
and compare those values for all gates and deltas. Workspace weighting uses
the persisted crate values. Display two decimal places. Persist raw metrics,
soft limits, and each penalty so a score can be explained and reproduced.
Fixed weights and metric semantics belong to the algorithm version; thresholds
are configurable, weights are not.

## Policy gates

An applicable crate fails below `minCrateScore`; the applicable workspace fails
below `minWorkspaceScore`. Any callable above `maxCyclomatic`,
`maxFunctionTokens`, or `maxNesting` fails even when its crate score passes.
Equality passes. Record each exceeded limit with its measured value, limit,
crate identity, and source span when applicable.

Compare each matching applicable crate and the workspace against the baseline.
A decrease greater than `maxRegression` points fails; improvements and equality
pass. The workspace comparison uses each report's own SLOC weights. Added crates
have no prior score and must pass absolute gates; removed crates are reported
as removed. Changes to applicability have nullable deltas. Crate identity is
the repository-relative manifest path, with package name as a display field;
renames at the same path retain identity.

Missing, malformed, unsupported, or policy-incompatible baselines fail health
with actionable diagnostics after measurement; they never erase a valid current
report. Compatibility requires matching snapshot schema, algorithm version,
effective scoring limits, duplicate window size, and source-selection policy
(manifest location and configured member selection). Absolute score floors,
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
paths, and effective health configuration. Run metadata holds revision and
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

The default check summary stays bounded and supplies a health retrieval hint
when health was measured. Health failures participate in ordinary failure
pagination and details; no per-crate success lines are added to normal output.

## Reviewed snapshots

`bloomery check health update [--run RUN] [--json]` atomically replaces the file
at `checks.codeHealth.baseline` with the selected run's complete current
measurement. It is the explicit review action for checking scores into version
control. A typical sequence is:

```text
bloomery check --check static:code-health
bloomery check health --run RUN
bloomery check health update --run RUN
```

Update can export a complete measurement whose policy gates failed, including
the initial missing-baseline failure. It requires a passed formatter outcome
and a finalized, uninterrupted run with complete health measurement; unrelated
check failures do not prevent export. Verify the current input digest against
the retained digest using source reads and discovery, without AST parsing,
formatting, or builds. Reuse the retained module graph for source paths;
changed graph-defining source or manifest bytes already invalidate its digest.
Changed sources, manifests, inventory, or effective
health configuration reject export as stale. Do not copy baseline-relative
fields or gate diagnostics into the snapshot: it stores current measurements
and their provenance, suitable for future comparison.

The checked-in `.bloomery/code-health-summary.json` is both the PR summary and
the comparison baseline. It uses schema version 3 and has top-level fields
`schema_version`, `algorithm_version`, `policy`, `input_digest`, `workspace`,
and `crates`. The workspace and each crate row expose `overall_score`,
production `sloc`, and category `subscores` for complexity, function size,
nesting, and duplication. Subscores are deduction points in each category.
Crate rows also include the display name and repository-relative `manifest_path`
used as their stable identity. Workspace score and subscores are SLOC weighted
from crate results. The policy retains only effective scoring limits and
source-selection inputs; the digest records which inputs produced the reviewed
result.

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
human or JSON output. Invalid requests exit 2; stale inputs or write failures
exit 1.

Ordinary checks and retrieval never modify the tracked baseline. Git or
jujutsu history of reviewed snapshots supplies the historical record; Bloomery
does not create commits or maintain a second repository history store.
