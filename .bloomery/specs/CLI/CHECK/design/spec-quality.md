# Specification quality

`static:traceability` assesses authored test links and explicit crate ownership
beside the [granularity policy](spec-granularity.md). The same specification
enablement gates apply. No policy launches tests or changes specifications.

## Untied tests

`checks.specifications.untiedTests` is `off`, `report` (default), or `deny`.
Report mode exposes untied sites in `spec trace --report` without failing check;
deny mode fails with `UntiedTestsLimitExceeded` when the number of nonexempt
distinct sites exceeds `maxUntiedTests` (default `0`). Equality passes.
Off disables this assessment; ordinary discovery and candidates remain intact.
Sites with orphan or manual references retain their existing findings and are
not untied. Identity uses scanner, canonical path, and declaration start.

`untiedExceptions` maps repository-relative path globs to a nonempty `reason`
and optional exact test `name`. A matching site remains visible with its reason
but does not consume the limit. Patterns use the source-tree glob semantics.
Canonical path matching works identically for filesystem and virtual inputs.
Overlapping matches use lexicographic pattern order. Exceptions do not hide
references, scanner errors, or other findings. `spec candidates` retains its
complete unfiltered graduation list.

## Crate ownership

`checks.specifications.crates` maps exact repository-relative Cargo manifest
paths to `features` containing exact `AREA/FEATURE` identifiers. Each mapping
defaults `minRequirements` and `minLinkedTests` to `1`; both allow zero.
Feature lists are nonempty and duplicate-free. Unknown feature identifiers and
unknown selected crate paths fail with `SpecificationPolicyError`.

Read workspace member manifests selected by `build.cargoToml` and optional
`build.members` crate names; honor declared workspace member patterns, exclusions, and a root
package. Ownership discovery inspects manifests only, with no Cargo invocation,
dependency resolution, or scanning of nested unrelated workspaces. Invalid or
unavailable selected manifests fail explicitly. With no configured mappings
and `requireCrateOwnership = false`, ownership is disabled and reads no Cargo
files. Otherwise every selected crate appears in the quality report.

For a mapped crate, report distinct declared automated requirements belonging
to its features and distinct authored validating sites linked to those
requirements, including integration tests outside that crate. A site linked to
several owned requirements counts once. These are feature associations, not
execution coverage. Compare counts with the crate's minima; violations are
`InsufficientCrateRequirements` and `InsufficientCrateTests`.

`requireCrateOwnership` defaults to `false`. When true, an unmapped selected
crate fails with `MissingCrateOwnership`; otherwise it remains visibly unowned.
A crate mapping may instead set `exclude = true` with a nonempty `reason` and
no features or minima. Excluded crates remain visible and skip ownership gates.
Exact manifest identities prevent package-name collisions and path-prefix
confusion. No ownership is inferred from a test's physical directory.

## Reports and completeness

`spec trace --report` includes nullable `quality` containing untied-test mode,
limit, every untied site and exception reason, crate associations, and discovery
completeness. Null means specifications or discovery were unavailable. Crate
rows are ordered by manifest; untied sites by scanner/path/line/column.
Configuration and findings participate in report identity under parser/report
contract 10; grouped requirement schema remains 1.

Observed excess counts can fail partial discovery. Below-minimum linked-test
findings require complete discovery; incomplete scope remains explicit. Crate
requirement minima use the independently complete specification set. Read and
parse failures never become an empty passing assessment. CLI checks assess the
configured evidence scope after successful scanning; virtual reports retain
their stricter scanner and eligibility completeness states.

All policy failures use ordinary bounded summaries, retained details, and exit
code 1. Exception shape, types, paths, IDs and nonempty reasons validate before
formatting, even with checks disabled. Repository-dependent targets validate
when specification analysis runs. Exception details remain reportable; no
policy silently removes authored facts.

## Configuration example

```toml
[checks.specifications]
enable = true
maxTestsPerRequirement = 5
untiedTests = "deny"
maxUntiedTests = 0
requireCrateOwnership = true

[checks.specifications.requirementExceptions.DEMO-API-INPUT-001]
maxTests = 8
reason = "Independent boundary cases for one input rule."

[checks.specifications.untiedExceptions."tests/support/**/*.rs"]
name = "fixture_roundtrip"
reason = "Checks the test fixture machinery."

[checks.specifications.crates."crates/api/Cargo.toml"]
features = ["DEMO/API"]
minRequirements = 3
minLinkedTests = 2

[checks.specifications.crates."crates/test-support/Cargo.toml"]
exclude = true
reason = "Test infrastructure only."
```
