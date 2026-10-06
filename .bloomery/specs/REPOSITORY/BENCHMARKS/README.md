---
id: BENCHMARKS
name: Bloomery Benchmark Subflakes
tagline: Compare Bloomery incremental build and check timings against ecosystem Nix Rust builders.
description: |
  The benchmarks feature defines a repository-local set of benchmark subflakes
  and a hyperfine driver that measure incremental Rust build and `nix flake
  check` timings for Bloomery and comparable ecosystem builders over two
  fixture groups: a simple single-dependency workspace and a standard axum and
  clap workspace. It covers alternative selection, fixtures, mutation
  scenarios, and the measurement harness. Benchmark execution is a local
  measurement workflow and is not part of `bloomery check`.
---

# Benchmark Subflakes

This feature records the repository's local benchmarking setup: two fixture
workspaces (`simple` and `standard`), one builder subflake per ecosystem
alternative, and a hyperfine harness that compiles the same scenarios through
each builder for each fixture group.

## Design documents

- [Alternative selection](design/alternatives.md)
- [Benchmark fixtures](design/fixtures.md)
- [Scenarios and mutations](design/scenarios.md)
- [Harness and reporting](design/harness.md)
- [Benchmark results](design/results.md)

## Scope

The feature owns the `benchmarks/` tree, including its subflake, the builder
subflake contract, the scenario catalog, the measurement harness, the
derivation expectations, and the benchmark result logs. The benchmarks subflake
exposes the `bench` runner app; the root flake exposes a second `bench` runner
app. Timing numbers
are observations, not pass/fail thresholds; automated validation covers harness
and fixture structure only. The reusable builders under test remain owned by
[NIXLIB](../../NIXLIB/README.md) and the ecosystem projects they wrap.
