# Dependency policy checks

Dependency policy checks evaluate each selected workspace member's resolved
dependency context. They join package identities, versions, and sources from
`Cargo.lock` with active edges and Cargo license metadata from `bloomery.lock`.
They do not compile crates or invoke Cargo, `cargo-audit`, or `cargo-deny`.

## Check selection

`checks.enable` gates the generated Nix checks. Each of
`checks.vulnerabilities.enable`, `checks.licenses.enable`, and
`checks.bans.enable` enables its own check family and defaults to `false`.
Bloomery generates one `<member>:vulnerabilities`, `<member>:licenses`, or
`<member>:bans` check for every selected workspace member whose corresponding
policy is enabled. A check covers that member's transitive dependency context,
excluding the member itself. `build.members` limits the members with checks;
`build.unify` does not change their dependency contexts.

Every dependency policy check requires a version-2 `bloomery.lock` whose
`cargo-lock-hash` matches the current `Cargo.lock`. This freshness requirement
applies even when `checks.lockFreshness.enable` is disabled. A missing, older,
or stale Bloomery lock fails evaluation with guidance to run `bloomery sync`.

Each policy check is a small Nix derivation over the evaluated graph and policy
result. Warnings are printed in the successful check log. Denied findings fail
the check derivation and print the member, offending package and version, and
an example dependency path from the member to that package.

## Vulnerability scan

The optional `rustsecAdvisoryDb` argument to `mkFlake` is a path to a pinned
RustSec advisory database source tree supplied by the consuming flake. The
argument is required when both `checks.enable` and
`checks.vulnerabilities.enable` are true and is not a config.toml value.
Bloomery reads the locked database during evaluation; it does not fetch
advisory data or invoke an audit executable.

Bloomery reads RustSec advisory records and compares their package names and
Cargo SemVer version ranges with registry packages in each member's resolved
context. It enumerates advisory Markdown files and parses their TOML front
matter in Nix, including supported RustSec v3 and v4 record shapes. A pure Nix
Cargo SemVer matcher evaluates the version selectors. RustSec records apply to
crates.io packages, so packages from workspace, Git, and other registry
sources do not match solely by name and version.
RustSec `patched` and `unaffected` ranges determine which versions are affected;
when neither range is present, every version is affected. Withdrawn advisories
are ignored. Matching is conservative and does not narrow findings by operating
system, architecture, or function-level reachability.

An exact ID in `checks.vulnerabilities.ignoreAdvisories` suppresses that
advisory. Other non-informational advisories deny the check. Informational
categories use
`checks.vulnerabilities.informational` values of `warn`, `deny`, or `ignore`.
`unmaintained`, `unsound`, `notice`, and unknown categories default to `warn`.
If an advisory has more than one informational category, the most severe
configured outcome applies: `deny`, then `warn`, then `ignore`. An unsupported
advisory schema or version selector fails evaluation rather than silently
omitting an advisory.

## License compatibility

`bloomery.lock` records each package's optional Cargo `license` expression and
`license-file` path from the metadata snapshot used to generate its graph. The
license check parses SPDX expressions in Nix from that metadata or an explicit
`checks.licenses.overrides` entry keyed by package selector. A license file by
itself is not an SPDX expression. A package without a declared expression or
override fails the check. A malformed package expression also fails the check;
an invalid configured expression is a configuration error.

`checks.licenses.allow` and `checks.licenses.deny` contain SPDX license IDs or
license-with-exception pairs. An empty `allow` list means there is no allowlist;
a non-empty list restricts accepted licenses. In an SPDX expression, `AND`
requires every branch to pass, `OR` passes when at least one branch passes, and
`WITH` is evaluated as a single license-and-exception pair. Deny rules exclude
matching branches. `checks.licenses.exceptions` lists package selectors that
are exempt from the global allow and deny lists; an exception does not supply a
missing license expression, so the package also needs a license override when
its metadata has no expression.

Package selectors match a canonical Cargo package name or an exact
`name@version`. In override table keys, selectors that are not valid TOML bare
keys are quoted. License override values are complete SPDX expressions and
take precedence over package metadata for the matching package.

## Package bans

`checks.bans.allow`, `checks.bans.deny`, and `checks.bans.exceptions` contain
package selectors. An empty `allow` list imposes no allowlist; a non-empty list
permits only matching packages. `deny` rejects matching packages even when
they also appear in `allow`. `exceptions` is applied last and permits a matching
package regardless of either list. A selector that matches a package reports
the dependency path in the affected member's check.

## Configuration

```toml
[checks.vulnerabilities]
enable = true
ignoreAdvisories = []

[checks.vulnerabilities.informational]
unmaintained = "warn"
unsound = "warn"
notice = "warn"
unknown = "warn"

[checks.bans]
enable = true
allow = []
deny = []
exceptions = []

[checks.licenses]
enable = true
allow = ["MIT", "Apache-2.0"]
deny = ["GPL-3.0-only"]
exceptions = []

[checks.licenses.overrides]
legacy-crate = "MIT OR Apache-2.0"
```
