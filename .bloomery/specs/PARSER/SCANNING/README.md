---
id: SCANNING
name: Static Evidence Scanning
tagline: Extract requirement references from source and Nix metadata without executing tests.
description: |
  The scanning feature parses configured Rust, Playwright, and Nix inputs and
  emits a common registry of requirement identifiers with source locations. It
  performs static inspection only and deliberately does not run test runners,
  binaries, or build derivations.
---

# Static Evidence Scanning

Scanning answers which requirement IDs are referenced by automated evidence. It
does not decide whether those IDs exist, whether they are manual, or whether
coverage is complete; those relational decisions belong to
[CLI/CHECK](../../CLI/CHECK/README.md).

## Design documents

- [Scanner architecture](design/overview.md)
- [Rust scanning](design/rust.md)
- [Playwright scanning](design/playwright.md)
- [Nix scanning](design/nix.md)

## Inputs and outputs

Configured source paths are parsed into a registry:

```text
Map<RequirementId, Vec<SourceLocation>>
```

Each source location retains a repository-relative file and a useful line or
span. Duplicate references from one source are retained only as needed for
diagnostics; the requirement-level registry is set-like.

## Safety boundary

The scanners may read configured files and, for Nix metadata, invoke the
configured evaluation operation. They do not execute test functions, launch
applications, instantiate derivations, fetch dependencies, or open network
connections.
