# Harness and reporting

## Entry point

The `benchmarks/` directory is its own flake. It exposes the `bench` app
(`nix run ./benchmarks#bench`). The root flake also exposes
`apps.<system>.bench` (`nix run .#bench`) that runs the same harness directly;
it does not reference the benchmarks flake, so the benchmark inputs stay out of
the root flake's closure. Both apps wrap `benchmarks/run.sh`, which drives every
builder and scenario using `hyperfine` from the pinned nixpkgs revision. Running
`benchmarks/run.sh` directly remains supported.

## Baseline snapshot

Each fixture group has one baseline snapshot under `benchmarks/.work`. The
snapshot is the canonical pre-mutation workspace. A run begins by restoring
every selected fixture from its snapshot, so a mutation left behind by an
interrupted run is discarded instead of becoming the next baseline. The
snapshot is created from the fixture only when it is missing; a normal run
never overwrites it. `--refresh-baseline` re-snapshots each selected fixture
from its current state after the fixtures are deliberately changed.

## Preparation

Preparation is untimed. Before each timed run the harness:

1. restores the fixture group's baseline workspace and generated inputs;
2. applies the scenario mutation;
3. regenerates Cargo and Bloomery locks and any builder-generated Nix; and
4. deletes the outputs of the derivations a mutation introduced, so the timed
   build cannot reuse a cached result.

Each fixture group's builder baseline package and checks are seeded into the
store once before its scenarios run, so unchanged dependencies stay warm.
Mutations add a unique nonce to affected sources (or a uniquely named member) so
every mutated derivation is new. Timed runs perform only the build or check
command.

## Hyperfine protocol

Each fixture, builder, and scenario runs as one hyperfine benchmark:

```text
hyperfine \
  --prepare 'benchmarks/harness/prepare.sh <fixture> <builder> <scenario>' \
  --warmup <n> --runs <n> --style basic --shell none \
  --export-json <out>.json \
  'benchmarks/harness/measure.sh <fixture> <builder> <scenario>'
```

The harness selects a fixture group with `--override-input workspace
path:benchmarks/fixtures/<group>`. The timed command runs only
`nix build benchmarks/flakes/<builder>#packages.<system>.default` for build
variants or `nix flake check benchmarks/flakes/<builder>` for check variants.
Output links are suppressed with `--no-link`.

## Store state

All fixtures and scenarios share one warm Nix store. Each fixture/builder
baseline package and checks are seeded once before its scenarios start, so
unchanged dependency artifacts stay cached. Source mutations and added members
produce new derivations; external dependency scenarios delete the outputs of the
derivations absent from the seeded baseline derivation closure. The timed build
therefore recompiles the changed crates and their dependents while reusing
shared artifacts. Fixed-output source fetches are never deleted. No garbage
collection runs during or between scenarios. Cold-store measurement is out of
scope.

Timed runs pass `--option substituters ""`, so Nix cannot substitute a cached
result for an evicted output. A timed run whose output reports a substitution
fails.

## Environment

Timed runs fix:

- the target system, narrowed to one platform;
- `--max-jobs` and `--cores`, so parallel widths match across builders;
- substituters are disabled and the store is pre-seeded so no substitution
  occurs during timing;
- `--override-input nixpkgs` to the repository revision; and
- locale and timezone, for stable tool output.

## Derivation logging

The harness captures each timed command's output and writes it to
`benchmarks/results/logs/<fixture>.<builder>.<scenario>.<variant>.log`. The
derivations Nix reported building are written to the sibling `.built` file and
announced on the console. Preparation output is written to the `.prep.log` file.

## Derivation expectations

`benchmarks/harness/derivations.json` records the derivation names each
fixture/builder/scenario/variant is expected to build. Names are normalised to
stable form: the store hash is dropped and the random `member-add` name becomes
`extra_<nonce>`. After each timed run the harness compares the built names
against the recorded expectation and fails on mismatch. `--record-derivations`
rewrites the expectations from a run.

## Report

The harness aggregates hyperfine JSON into a table with one row per builder and
scenario. Each row carries the sample count, mean, standard deviation, minimum,
and maximum wall time, plus the ratio to Bloomery for that scenario. Results are
written to the files described in [Benchmark results](results.md).

The report header records:

- Bloomery revision and alternative input revisions;
- the nixpkgs revision, Rust toolchain version, and target system;
- CPU model, logical and physical core counts, and total memory;
- `--max-jobs` and `--cores`; and
- the preparation steps that ran.

A builder that fails to build or evaluate produces a non-zero harness exit and a
reported failure. It is never reported as a zero or missing measurement.

## Reproducibility

Every input is pinned, preparation is deterministic, and timed runs use no
substituters. The report carries enough metadata to compare runs and to explain
environment differences.