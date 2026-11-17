---
id: PLAYWRIGHT
name: Sandboxed Browser Tests
tagline: Execute Playwright suites against local servers in Nix check derivations.
description: |
  The Playwright feature owns the reusable browser-test constructor, pinned
  browser tooling, server lifecycle, and check reports.
---

# Browser tests

`perSystemWorkspace.<system>.playwrightTest` constructs browser checks for
`mkFlake.extraOutputs`. [Runner design](design/runner.md) defines the contract.
