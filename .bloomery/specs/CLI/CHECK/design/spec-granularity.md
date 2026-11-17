# Specification granularity

`static:traceability` fails with `TooManyTestsForRequirement` when an automated
requirement has more distinct validating sites than
`checks.specifications.maxTestsPerRequirement`. The effective default is `5`;
equality passes. The unit is one requirement ID, not a group, feature, or design
document. Counts combine every enabled scanner's configured evidence scope.
Specification management and `checks.specifications.enable` gate the policy
with the other traceability rules. Selecting other checks does not run it.

## Counting

Count authored associations, using these static units:

| Scanner | One validating site |
| --- | --- |
| Rust | One test function declaration. |
| Playwright | One recognized `test(...)` or `test.describe(...)` call carrying a direct requirement tag. |
| Nix | One file carrying literal `passthru.bloomery` references in the configured evidence scope. |

Rust and Playwright identity is scanner kind, canonical repository-relative
path, and declaration start line/column. Nix identity is scanner kind and path;
`scanners.nix.testPaths` controls candidate discovery, not this file count.
Distinct declarations with the same name count separately. Duplicate IDs,
multiple annotations on one declaration, overlapping globs, and repeated
discovery of the same site count once per requirement. A site referencing
several requirements contributes once to each requirement.

Parameterized rows, loop iterations, macro expansions, retries, feature
matrices, target matrices, and selected Nix systems do not multiply sites.
Playwright suite tags count at their authored call; they are not expanded onto
descendants. Multiple Nix checks in one file remain one static unit.

This measures authored coupling, not executed or passing tests. Rust cfg,
target, and ignored-test eligibility do not remove authored associations from
this count; report coverage retains its separate eligibility rules. A limit
violation suggests reviewing requirement scope, but does not prove that the
requirement combines different obligations. Boundary cases and independent
test layers can legitimately exercise one narrow obligation many times.

## Validation

Apply the rule only to declared automated requirements. Unknown and manual
IDs retain their existing orphan and illegal-manual diagnostics without an
additional granularity finding. Preserve the missing-test rule independently.
Shared relational validation, CLI checks, and filesystem and virtual static
reports use the same site identities and threshold semantics. Caller-supplied
analysis facts retain site ownership; reference literal locations alone cannot
distinguish duplicate annotations from distinct tests.

Partial discovery gives a lower bound: emit a violation when observed distinct
sites already exceed the limit, but never treat a count at or below the limit
as proof about unavailable sources. Scanner read and parse failures retain
their failure status; unsuccessful discovery cannot yield a passing
granularity assessment. Static reports preserve discovery and relational
completeness separately from these findings.

## Diagnostics

Emit one `TooManyTestsForRequirement` finding per over-limit requirement at its
declaration. Include the requirement ID, observed distinct-site count, effective
limit, and every contributing site's scanner and repository-relative location
in deterministic scanner/path/line/column order. Notes recommend reviewing the
obligations and splitting independent stories while preserving meaningful
tests. Never remove tests or rewrite requirements automatically.

Retain complete findings in the ordinary run store. Human and JSON summaries
use existing failure bounds and paging; `check details` exposes all contributing
sites. A selected check with a granularity violation fails with exit code `1`.
An exact requirement ID in `checks.specifications.requirementExceptions`
overrides its limit through `maxTests`, with a nonempty `reason`. Overrides
name existing automated requirements and survive configuration upgrades.
Source suppression annotations are not supported.
