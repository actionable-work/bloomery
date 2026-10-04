---
id: SOURCES
name: Nix Source Isolation
tagline: Rebuild only derivations whose declared inputs change.
description: |
  Source selection gives each Rust derivation a stable, filtered input tree.
  Compilation, checks, assets, and repository test support depend on selected
  content rather than the identity of the enclosing flake snapshot. The same
  contract applies to direct workspace construction and both flake integrations.
---

# Source Isolation

Filesets define build inputs, not a blanket file-extension exclusion policy.
Markdown embedded in Rust, test fixtures, and installed assets are legitimate
inputs; unrelated prose and repository metadata are not.

## Design documents

- [Fileset selection](design/filesets.md)
- [Derivation identity and rebuild boundaries](design/identity.md)

## Scope

This feature covers workspace member sources, auxiliary repository inputs,
bundled assets, and the generated release/dev packages, unit/integration tests,
clippy, rustdoc, and doctests. It depends on workspace discovery, resolved lock
metadata, and the existing override mechanism.

Whole-crate filtering is the default granularity; per-module or per-binary
reachability analysis is not required. Arbitrary build-script or macro reads
are supplied through explicit filesets or source overrides, not inferred by
parsing Rust. External registry archives and pinned Git dependencies retain
their upstream source contracts.

Checks that inspect prose, specifications, formatting, or repository layout may
legitimately change when their own selected inputs change. This feature does
not promise that every flake check remains unchanged after a Markdown edit.
