# bloomery

A pure Nix library for building Rust applications and workspaces directly from `Cargo.lock` and `rustc`, completely bypassing Cargo during builds. The Bloomery CLI separately provides `bloomery sync` to reconcile lockfiles before evaluation.

Inspired by `oxalica/nocargo`, this flake:
- **Direct `Cargo.lock` parsing**: Reads package versions and dependency trees purely in Nix using `builtins.fromTOML` (no external code generation).
- **Zero Git index inputs**: Eliminates the multi-gigabyte `crates.io-index` git repository input entirely.
- **Isolated resolution via `bloomery.lock`**: The `bloomery sync` CLI generates a static, lightweight lock manifest pinning active features and dependencies for instant (<1ms) evaluation.
- **Direct source fetching with zero IFD**: Downloads crates.io dependencies via `pkgs.fetchurl` using the SHA-256 checksums already recorded in `Cargo.lock`.
- **Pure `rustc` compiler driver**:
  - Compiles `.rlib` dependencies and `proc-macro` crates independently.
  - Automatically compiles and executes build scripts (`build.rs`), capturing generated environment variables, `cargo:rustc-cfg`, and native library link search paths.
  - Links rlib dependencies and workspace binaries using `rustc --crate-type bin` with default high-performance linkers (`lld` on Linux).
- **Checks without cargo**: Generates independent CI checks for unit tests (`rustc --test`), clippy (`clippy-driver`), documentation (`rustdoc`), doctests (`rustdoc --test`), package builds, and lock validation. Run `bloomery check` separately to combine static specification validation with the selected Nix checks; generated checks never invoke the top-level runner recursively.
- **Built-in DevShell & Apps**: Every workspace automatically generates a development shell (with `rustc`, `cargo`, `clippy`, `nix-fast-build`, the Bloomery CLI, direnv support) and runnable apps for its binaries and documentation.

---

## Consumer API

Bloomery exposes a single constructor, `bloomery.mkFlake`. It reads every build
setting from a required `.bloomery/config.toml` at the workspace root.

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    bloomery.url = "github:actionable-work/bloomery";
  };

  outputs = { nixpkgs, bloomery, ... }:
    bloomery.mkFlake {
      inherit nixpkgs;
      root = ./.;
    };
}
```

`mkFlake` accepts `nixpkgs`, `root`, an optional `systems` list, an optional
`overrides` set, an optional `extraOutputs` callback, and an optional `self`.
It does not accept workspace options as arguments.

```toml
# .bloomery/config.toml (required)
[build]
libPackages = false
devPackages = true
profileName = "release"

[toolchain]
linker = "lld"

[profile.release]
optLevel = 3
lto = "thin"
codegenUnits = 1

[flags]
doc = ["-Dwarnings"]

[devShell]
enable = true

[checks]
enable = true
includePackageChecks = true

[features]
unify = true
```

Package-valued settings are nixpkgs attribute paths; path-valued settings are
relative to the workspace root. The full table reference is in the
[API documentation](./packages/rust/libs/docs/content/src/docs/api.md).

`mkFlake` generates `packages`, `apps`, `checks`, and `devShells` across the
selected systems (`x86_64-linux`, `aarch64-linux`, and `aarch64-darwin` by
default).

Synchronize locks from a workspace with the Bloomery CLI package, for example
`nix run github:actionable-work/bloomery#bloomery -- sync`. `bloomery sync`
reconciles `Cargo.lock` and writes `bloomery.lock`; use `--update=rust`,
`--update=nix`, or bare `--update` to request ecosystem updates. Sync requires
`.bloomery/config.toml` and does not create or edit it. The `check`, `review`,
and `sync` commands share `--json` for machine-readable output; human terminal
output is colorized automatically and honors `NO_COLOR`.

### Per-Crate Overrides

Overrides carry arbitrary Nix values, so they stay in Nix. Pass them through the
`overrides` argument or place an `overrides.nix` file next to any workspace
member's `Cargo.toml`:

```nix
bloomery.mkFlake {
  inherit nixpkgs;
  root = ./.;
  overrides.openssl-sys = {
    nativeBuildInputs = [ pkgs.pkg-config ];
    buildInputs = [ pkgs.openssl ];
    rustcFlags = [ "-Ctarget-cpu=native" ];
    env = { CUSTOM_ASSETS = "./assets"; };
  };
}
```

```nix
# crates/my-app/overrides.nix
{ pkgs, lib, ... }: {
  fileset = lib.fileset.unions [
    ./src
    ./Cargo.toml
    ./templates
    ./assets
  ];
  nativeBuildInputs = [ pkgs.pkg-config ];
  buildInputs = [ pkgs.openssl ];
}
```

---

## Workspace Outputs

`mkFlake` returns, per selected system:

- `packages`: Derivations for workspace member binaries, plus optional library and dev outputs, and `default` when a binary exists.
- `apps`: Runnable app specifications for binaries, optional dev binaries, documentation, and `default` when a binary exists.
- `checks`: Independent CI derivations (`crate:test`, `crate:clippy`, `crate:doc`, `crate:doctest`, `crate:bin`, `crate:lib`, and `workspace:lock`). They do not include a recursive `bloomery:check` output; invoke `bloomery check` directly for static validation and orchestration.
- `devShell`: Preconfigured development shell with rustc, clippy, cargo, nix-fast-build, the Bloomery CLI, and lld (null when disabled).
- `crates`: DAG attribute set of all built `.rlib` crates.
- `lock`: Parsed lockfile representation.
- `config`: Evaluated and type-checked options.

---

## Test Workspaces (Setup Styles)

Every supported setup style is verified by an isolated sub-flake in CI:

| Workspace | Setup Style | Description |
|---|---|---|
| [`tests/basic-workspace`](./tests/basic-workspace) | Config-driven `mkFlake` | Common five-member workspace layout with `build.rs`, unit tests, doctests, and crates.io dependencies |
| [`tests/axum-workspace`](./tests/axum-workspace) | Config-driven `mkFlake` | Axum webserver and CLI, including server asset packaging |
| [`tests/single-crate-workspace`](./tests/single-crate-workspace) | Nix `overrides` argument | Single-crate workspace exercising the `overrides` argument |
| [`tests/flake-parts-workspace`](./tests/flake-parts-workspace) | Internal flake-parts module | Exercises the internal flake-parts module directly; the public API is `mkFlake` |
| [`tests/edge-cases-workspace`](./tests/edge-cases-workspace) | Config-driven `mkFlake` | Root package, glob members, excluded crates, build cfgs, integration tests, and multiple binary layouts |
| [`tests/overrides-workspace`](./tests/overrides-workspace) | Multi-Crate Overrides | Validates colocated member overrides and flake-level overrides merging in a multi-crate workspace |
