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
declares. A dependency is referenced when its Rust crate name appears as an
identifier token anywhere in those sources, including paths, `use` and
`extern crate` items, attributes, macro bodies, and conditionally compiled
code. The crate name is the manifest rename, else the dependency library target
name when the dependency is a workspace package, else the package name, with
hyphens replaced by underscores.

Visibility follows the declaring section:

| Section | Referencing targets |
| --- | --- |
| `dependencies` | library, binary, and proc-macro targets |
| `dev-dependencies` | every target except build scripts |
| `build-dependencies` | build scripts |

Platform-specific `[target.*]` sections follow the same rules. A normal
dependency referenced only by tests, examples, or benchmarks is unused; it
belongs in `dev-dependencies`.

`checks.unusedDependencies.exceptions` lists dependency package names that are
never reported, such as crates linked only for their side effects. Exceptions
match the package name, not a manifest rename.

## Diagnostics

Each unused declaration produces one `UnusedDependency` diagnostic naming the
member package, section, and manifest key, located at the repository-relative
member `Cargo.toml` and the declaring key's line. Its note names the Rust crate
name that was searched for.

Metadata discovery failures and unreadable or unparsable target sources produce
`UnusedDependencySourceError`. A member with a source failure reports only its
source failures, never unused declarations inferred from incomplete sources.
