# Unused dependencies

## Check integration

`static:unused-dependencies` inspects every workspace member of the configured
Cargo workspace, discovered through offline `cargo metadata --no-deps`. It is
opt-in through `checks.unusedDependencies.enable`, which defaults to `false`.
Its enablement is independent of specification management, Rust evidence
scanning, Nix check generation, and selected systems. It follows ordinary check
selection, scheduling, cancellation, failure retention, and exit code behavior.
Disabled policy contributes no catalog entry.

The check runs after the write-mode formatting preflight. It is a separate
static check from code health and the Nix dependency policy checks, and it
changes no manifests, lockfiles, or sources.

## Usage policy

Each target's sources are its root file and every out-of-line module file it
declares. A dependency is referenced when its Rust crate name starts a path
(`name::`), follows a leading `::`, or is the root of a `use` tree or an
`extern crate` item anywhere in those sources, including attributes, macro
bodies, and conditionally compiled code. A leading `::` follows punctuation
other than `>`, `->`, `=>`, or a keyword other than `self`, `Self`, `super`,
and `crate`. Each entry of a `use { ... }` or
`use ::{ ... }` group is a `use` tree root. Identifiers in other positions,
such as locals, fields, keywords, and segments after `::` or `.`, are not
references. A relative `name::` path is not a reference when its module
scope declares `mod name`: a file or inline module body is a new scope, and
other blocks share the enclosing one. Documentation text, including doctests,
references a crate when it starts a `name::` or `::name` path with the crate
name, or names it after a line-leading `use` or after `extern crate`;
documentation references are test-only. The crate name is the manifest rename, else the dependency library
target name when the dependency is a workspace package, else the package name,
with hyphens replaced by underscores.

Code confined to `cfg(test)` is test-only: an item whose `cfg` predicate holds
only when `test` is set, through the end of its body or `;` (or, for a field,
variant, arm, or argument, its `,` outside generic arguments), a module file declared by such an item, and a file
whose inner attributes require `cfg(test)`. Test-only code references only
dev-dependencies.

Visibility follows the declaring section:

| Section | Referencing targets |
| --- | --- |
| `dependencies` | non-test code of library, binary, and proc-macro targets |
| `dev-dependencies` | every target except build scripts |
| `build-dependencies` | build scripts |

Platform-specific `[target.*]` sections follow the same rules. A normal
dependency referenced only by tests, examples, benchmarks, or test-only code is
unused; it belongs in `dev-dependencies`.

The compiler-backed
[`<crate>:unused-dependencies`](../../../NIXLIB/GRAPH/design/unused-dependency-checks.md)
Nix checks apply the same visibility rules to the units rustc compiles.

`checks.unusedDependencies.exceptions` lists dependency package names that are
never reported, such as crates linked only for their side effects. Exceptions
match the package name, not a manifest rename.

## Diagnostics

Each unused declaration produces one `UnusedDependency` diagnostic naming the
member package, section, and manifest key, located at the repository-relative
member `Cargo.toml` and the declaring key's line. Its note names the Rust crate
name that was searched for and the repairs: removing or moving the entry, a
`use name as _;` item for a dependency needed only for linking, or an
exception.

Metadata discovery failures and unreadable or unparsable target sources produce
`UnusedDependencySourceError`. A member with a source failure reports only its
source failures, never unused declarations inferred from incomplete sources.
