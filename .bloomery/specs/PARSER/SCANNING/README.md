---
id: SCANNING
name: Static Evidence Scanning
tagline: Extract requirement references and discover test sites from source without executing tests.
description: |
  The scanning feature parses configured Rust, Playwright, and Nix inputs and
  emits requirement references with source locations alongside the discovered
  test sites that carry them. It performs static inspection only and
  deliberately does not run test runners, binaries, or build derivations.
---

# Static Evidence Scanning

Scanning answers which requirement IDs are referenced by automated evidence and
which tests exist in configured source. It does not decide whether referenced
IDs exist, whether they are manual, or whether coverage is complete; those
relational decisions belong to [CLI/CHECK](../../CLI/CHECK/README.md). The
candidate projection over untagged tests belongs to
[CLI/SPEC](../../CLI/SPEC/README.md).

## Design documents

- [Scanner architecture](design/overview.md)
- [Rust scanning](design/rust.md)
- [Playwright scanning](design/playwright.md)
- [Nix scanning](design/nix.md)
- [Test discovery](design/test-discovery.md)

## Inputs and outputs

Configured source paths are parsed into two related views:

```text
Map<RequirementId, Vec<SourceLocation>>   references
Vec<TestSite>                             discovered tests
```

Each source location retains a repository-relative file and a useful line or
span. Duplicate references from one source are retained only as needed for
diagnostics; the requirement-level registry is set-like. Each test site retains
its optional name, source location, and the requirement references extracted
from that site. Nix test sites are files matched by `scanners.nix.testPaths`.

Within one command, a source file is parsed at most once; reference extraction
and test discovery share one command-scoped parse cache. See the
[parse-once contract](design/overview.md#parse-once).

## Safety boundary

The scanners may read configured files and, for Nix metadata, invoke the
configured evaluation operation. They do not execute test functions, launch
applications, instantiate derivations, fetch dependencies, or open network
connections.
