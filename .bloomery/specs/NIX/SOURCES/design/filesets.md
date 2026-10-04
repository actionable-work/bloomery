# Fileset selection

## Default member sources

Each member has a fileset rooted at its crate directory and materialized with
`lib.fileset.toSource`. Selection is based on the member's build inputs rather
than subtraction of a few generated directories from the entire crate tree.

The common compilation fileset includes:

- The member `Cargo.toml` and Rust sources, including root-level, symlinked,
  and manifest-declared entrypoints.
- Complete source directories containing those entrypoints, preserving relative
  layout and non-Rust files used by `include_str!`, `include_bytes!`, and macros.
- Sibling module directories resolved from root-level entrypoints (`mod
  helpers;` selects `helpers.rs` or the complete `helpers/` tree), without
  selecting the package root.
- Crate-local `assets`, `static`, `public`, and configured `assetDirs` consumed
  by the builders.

A supported entrypoint that is a symlink is selected like any other entrypoint.
Because filesets preserve symlinks but cannot select their targets, and
unresolved targets would leave the build source dangling, the build source also
materializes each link's dereferenced content at the link's in-crate target
path. Rust resolves nested modules relative to the link location, so the
resulting tree keeps the effective module layout and never selects the
enclosing package root.

Test derivations additionally include their integration-test sources and fixture
inputs. Test-only trees and auxiliary inputs do not enter production compilation
or packaging unless explicitly declared as common inputs. Clippy, rustdoc, and
doctests receive the inputs consumed by their respective builder phases.

Unreferenced crate-root README files, standalone documentation, Bloomery
specifications and run data, VCS metadata, editor state, build outputs, result
links, and development-environment state are excluded from default Rust inputs.
A root package follows the same policy as a nested member; the workspace root
is not implicitly a source tree for every package. Other member trees are not
recursively included merely because they reside beneath a root package.

Keep all files inside a selected source, fixture, or asset directory regardless
of extension. Extension-only Rust filtering is insufficient, and blanket
Markdown exclusion is incorrect.

## Explicit selection

Source precedence is explicit `src`, then explicit `fileset`, then the default
member fileset. A custom fileset replaces the default selection; callers include
all necessary inputs and preserve the required crate-relative layout. Explicit
selection can include otherwise excluded files, including Markdown and data
outside conventional source directories. Filesets are materialized relative to
a containing root that preserves the builder's crate-relative paths.

An explicit source derivation is authoritative and is not re-filtered. An
explicit local path selects that subtree, not its enclosing flake snapshot;
its inclusion policy is caller-owned. Accepted local inputs are path values or
context-bearing absolute strings; both are materialized as isolated trees with
a stable name. General-purpose dependency, flag, and environment overrides
remain common inputs. Test-only repository support uses phase-specific inputs
rather than common overrides.

Library and binary entrypoint availability follows the effective selected
source, including custom fileset membership, so an explicit override may add or
remove a member's library entrypoint. An opaque source derivation falls back to
the raw member manifest.

Each member override exposes a `test` namespace with `nativeBuildInputs`,
`buildInputs`, `env`, and an additive fixture `fileset`. Default, colocated,
and user test settings merge independently of the common settings and overlay
only the test builder. The additive fileset unions with the common compilation
selection; an explicit full `src` or `fileset` remains caller-owned.

## Assets and auxiliary trees

Workspace `assets`, `static`, and `public` are independently materialized source
trees. Explicit local asset paths likewise select only their own contents.
Only derivations that consume or install an asset depend on that asset tree.

Repository test-support trees select exactly the files inspected by the tests:
layout support selects the workspace manifest and binary entrypoints;
Nix API migration support selects the API files those tests inspect. Their
builders use filtered roots before interpolating any path into shell commands,
environment variables, or symlink targets. A selected subpath of an unfiltered
flake root is not an isolated input. Identity coverage imports these actual
overrides against paired enclosing snapshots so unrelated prose preserves
support and consumer identities while edits to selected support files change
consuming tests only.
