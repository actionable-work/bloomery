# Inline Clippy suppressions

## Check integration

`static:clippy-suppressions` scans Rust source files in the configured Cargo
workspace, including library, binary, example, test, benchmark, and build-script
sources. It is opt-in through `checks.clippySuppressions.enable`, which defaults
to `false`. Its enablement is independent of specification management, Rust
evidence scanning, Nix check generation, and selected systems. It follows
ordinary check selection, scheduling, cancellation, failure retention, and exit
code behavior. Disabled policy contributes no catalog entry.

The check runs after the mandatory formatting preflight. It is a separate static
check from code health and does not affect health scores or the Nix Clippy
derivations.

## Suppression policy

Inspect inline Rust attributes that disable a Clippy lint, including outer and
inner `allow` attributes and `cfg_attr`-wrapped `allow` attributes. Report each
Clippy lint name not present in `checks.clippySuppressions.allow`. A configured
lint name permits that exact name wherever it is suppressed; group names such
as `clippy::all` are exact names and do not implicitly permit their members.
Other Rust lint namespaces and generic `warnings` allowances are outside this
policy.

Diagnostics identify the repository-relative source path, attribute line, and
suppressed lint name. Multiple disallowed names in one attribute produce one
diagnostic per name. No source files or configuration are changed by the check.
