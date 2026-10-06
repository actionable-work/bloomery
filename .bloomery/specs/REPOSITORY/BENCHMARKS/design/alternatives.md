# Ecosystem alternatives

The benchmark compares Bloomery against builders that represent the distinct
Nix Rust build strategies in the ecosystem.

## Selected builders

| Builder | Strategy | Nix incrementality |
| --- | --- | --- |
| Bloomery | pure Nix `rustc` derivations from lock data | one derivation per crate |
| crane | layered cargo derivations with cached dependency artifacts | dependency layer cached; workspace members rebuilt together |
| cargo2nix | generated `Cargo.nix` from `Cargo.lock` | one derivation per crate |
| crate2nix | generated `Cargo.nix` from `Cargo.lock` | one derivation per crate |
| naersk | one cargo derivation for the whole workspace | whole workspace per derivation |

`cargo2nix` and `crate2nix` are codegen builders; their generated expressions
are produced during untimed preparation. `crane`, `naersk`, and `crate2nix`
drive `cargo`; Bloomery invokes `rustc` directly. Every builder compiles with the
nixpkgs toolchain version: the cargo-based builders use it directly, and
`cargo2nix` selects it through `rust-overlay`.

## Selection criteria

A builder is selected when it:

- builds a Cargo workspace from `Cargo.toml` and `Cargo.lock`;
- adds a distinct incrementality strategy or is a widely adopted baseline;
- can be pinned to an exact input revision; and
- evaluates and builds against the benchmark's nixpkgs revision.

## Fairness constraints

All builders share:

- one nixpkgs revision, pinned in the benchmark lockfile and forced with
  `--override-input` so every subflake evaluates the same package set;
- one Rust toolchain version, taken from that nixpkgs revision, unless the
  builder pins its own toolchain, in which case the pinned version is matched or
  the deviation is recorded in the report;
- one fixture source tree, one `Cargo.toml`, and one `Cargo.lock`;
- one Cargo profile, feature selection, target platform, and linker;
- one Nix configuration: fixed `--max-jobs` and `--cores`, sandbox enabled,
  fixed substituters, and no network access during timed runs; and
- one warm store seeded with baseline and dependency artifacts.

Code generation and lock synchronization are preparation, not build time. The
report records that preparation ran.

## Optional baselines

`rustPlatform.buildRustPackage` from the pinned nixpkgs is an optional added
baseline. It uses the same builder contract and fixture as the selected
builders.

## Excluded builders

- `rust-flake` wraps crane and adds a development workflow rather than a
  distinct build strategy.
- `dream2nix`'s Rust builder is cargo-based and overlaps naersk while owning a
  much broader multi-language scope.
- `fenix` supplies toolchains rather than workspace builds.

## Check granularity

Each builder runs its idiomatic `nix flake check` suite. Bloomery exposes
individual per-crate test, clippy, doc, and doctest checks. crane exposes
workspace-level clippy and test derivations. cargo2nix, crate2nix, and naersk
expose the checks their integrations support. The benchmark compares
total check wall time, not per-crate parity.
