# Rust ignored-test eligibility

Rust condition facts retain the test function's own cfg/ignore attributes beside
its inherited compilation gates. Bare `ignore`, string reasons and nested
`cfg_attr` syntax share the existing parser cache and source locations.
Literal references and test sites remain discoverable regardless of eligibility.

Static reports annotate Rust tests and bindings with `rust_harness` eligibility
in both syntactic and explicit cfg scope. Plain ignore attributes and potentially
active conditional ignore attributes are unresolved with a stable reason.
Conditional predicates use only caller facts, or unknown facts when no context
is supplied. Provably inactive ignore attributes allow eligibility. Malformed or
unsupported conditions remain unresolved. Ignore attributes on enclosing files
or modules do not apply to child tests.

Harness eligibility is separate from compilation cfg and selected target
membership. Only satisfied harness eligibility can credit Rust binding coverage.
Unresolved eligibility makes relational analysis partial and absent coverage
unresolved. An independently eligible binding can still credit its requirement.
Orphan and manual-reference diagnostics retain literal reference spans.

`completeness.rust_harness` independently records complete, partial, disabled,
failed or unattempted analysis. This projection does not select runner options,
execute tests or establish execution success. Raw scanner evidence remains a
syntactic discovery API; execution configuration and hosted policy stay outside
the static report. Report/parser contracts are 8; specification schema remains 1.
