# Scenarios and mutations

## Scenario model

Every scenario runs against every fixture group. A scenario is a named pair of
a baseline workspace state and a mutation applied to that state. Each scenario
declares:

- `id`: the scenario name used in commands and reports;
- `target`: the crate, manifest, or lock input the mutation edits, relative to
  the fixture workspace;
- `mutation`: the exact edit applied before a timed run;
- `affected`: the crates expected to rebuild; and
- `variants`: whether the scenario is measured as a build or a check.

Scenario targets are fixture-agnostic: both groups expose `crates/app` and
`crates/util`.

## Scenario catalog

Build variants time `packages.<system>.default`. Check variants time the
builder's full `nix flake check`.

| Scenario | Mutation | Affected | Variants |
| --- | --- | --- | --- |
| `no-change` | none | none | build, check |
| `member-source` | nonce edit to `crates/app/src/main.rs` | `app` | build, check |
| `member-dependency-source` | nonce edit to `crates/util/src/lib.rs` | `util` and its dependents | build, check |
| `registry-dependency` | change the `[workspace.dependencies]` crates.io version | changed registry crate and all dependents | build, check |
| `member-add` | add a new workspace member and wire it into `app` | `app` and the new member | build |

`no-change` establishes evaluation and warm-store overhead. `member-source`
isolates single-crate incrementality. `member-dependency-source` measures a
change to a workspace member used as a dependency. `registry-dependency`
measures external dependency invalidation. `member-add` measures graph growth.

## Mutation contract

Mutations are deterministic and confined to their declared target. A mutation
must not change the Cargo profile, feature selection, linker, target platform,
or any dependency outside its declared target.

Preparation, not the timed command, applies every mutation. Source mutations
append a unique nonce comment and the added member gets a unique name, so their
mutated derivations are new. The external dependency scenario changes the
registry version and deletes the changed derivations from the store, keeping
fixed-output source fetches and all unchanged dependency artifacts cached. No
timed build can reuse a cached result. The baseline reset removes the previous
mutation before the next preparation.

Preparation restores the baseline workspace, applies the mutation, and
regenerates every derived input before the next timed run:

- `Cargo.lock` and `bloomery.lock`;
- generated Nix for cargo2nix and crate2nix; and
- any builder-specific lock or generated file.

Preparation is untimed; the timed command performs only the build or check. The
report records which preparation steps ran for each scenario.

## Check scenarios

Check variants run the builder's complete check suite:

- Bloomery runs its individual per-crate test, clippy, doc, doctest, and package
  checks.
- crane runs workspace-level clippy and test derivations.
- Cargo-based and codegen builders run the checks their integration exposes.

The measured quantity is the full check wall time after the scenario mutation,
including Nix evaluation and realization of the checks that the mutation
invalidates.
