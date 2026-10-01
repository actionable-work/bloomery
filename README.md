# bloomery

A pure Nix library for building Rust applications and workspaces directly from `Cargo.lock` and `rustc`, completely bypassing `cargo`.

Inspired by `oxalica/nocargo`, this flake:
- **Direct `Cargo.lock` parsing**: Reads package versions and dependency trees purely in Nix using `builtins.fromTOML` (no external code generation).
- **Zero Git index inputs**: Eliminates the multi-gigabyte `crates.io-index` git repository input entirely.
- **Isolated resolution via `bloomery.lock`**: Generates a static, lightweight lock manifest (`nix run bloomery#lock`) pinning active features and dependencies with instant (<1ms) evaluation.
- **Direct source fetching with zero IFD**: Downloads crates.io dependencies via `pkgs.fetchurl` using the SHA-256 checksums already recorded in `Cargo.lock`.
- **Pure `rustc` compiler driver**:
  - Compiles `.rlib` dependencies and `proc-macro` crates independently.
  - Automatically compiles and executes build scripts (`build.rs`), capturing generated environment variables, `cargo:rustc-cfg`, and native library link search paths.
  - Links rlib dependencies and workspace binaries using `rustc --crate-type bin` with default high-performance linkers (`lld` on Linux).
- **Checks without cargo**: Generates independent CI checks for unit tests (`rustc --test`), clippy (`clippy-driver`), documentation (`rustdoc`), and doctests (`rustdoc --test`).
- **Built-in DevShell & Apps**: Every workspace automatically generates a development shell (with `rustc`, `cargo`, `clippy`, `nix-fast-build`, direnv support) and a `lock` app.

---

## Consumer APIs

Bloomery supports three consumption workflows:

### 1. Zero-Boilerplate (`bloomery.mkFlake`)

The fastest way to package a Rust workspace:

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

This automatically generates `packages`, `apps`, `checks`, and `devShells` across standard systems (`x86_64-linux`, `aarch64-linux`, `aarch64-darwin`).
The generated `apps.lock` runs Bloomery's lock generator; invoke it with `nix run .#lock` from the workspace root.

### 2. Standard Flake (`bloomery.lib.${system}.mkWorkspace`)

When you want manual control over system outputs:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    bloomery.url = "github:actionable-work/bloomery";
  };

  outputs = { nixpkgs, bloomery, ... }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ];
      eachSystem = nixpkgs.lib.genAttrs systems;
    in {
      packages = eachSystem (system:
        (bloomery.lib.${system}.mkWorkspace {
          root = ./.;
        }).packages
      );

      apps = eachSystem (system:
        (bloomery.lib.${system}.mkWorkspace {
          root = ./.;
        }).apps
      );

      checks = eachSystem (system:
        (bloomery.lib.${system}.mkWorkspace {
          root = ./.;
        }).checks
      );

      devShells = eachSystem (system: {
        default = (bloomery.lib.${system}.mkWorkspace {
          root = ./.;
        }).devShell;
      });
    };
}
```

*Tip*: If you have custom `pkgs` with overlays, construct the library directly with `bloomery.mkLib pkgs`.

### 3. Flake-Parts Module (`bloomery.flakeModules.default`)

Integrate into a `flake-parts` project with strongly-typed module options:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    bloomery.url = "github:actionable-work/bloomery";
  };

  outputs = inputs@{ flake-parts, bloomery, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ bloomery.flakeModules.default ];
      systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ];
      perSystem = { ... }: {
        bloomery.workspace = {
          root = ./.;
          profile = {
            optLevel = 3;
            lto = "thin";
          };
        };
      };
    };
}
```

---

## Categorized `mkWorkspace` Options

Workspace configuration is strongly typed and organized into clear categories:

```nix
bloomery.lib.${system}.mkWorkspace {
  # Top-level required setting
  root = ./.;

  # ── Public Package Outputs ────────────────────────────────────────────────
  createLibPackages = false;             # expose <crate>:lib outputs
  createDevPackages = false;             # expose <name>:dev apps

  # ── Source & Files ────────────────────────────────────────────────────────
  source = {
    cargoToml = ./Cargo.toml;           # defaults to root + "/Cargo.toml"
    cargoLock = ./Cargo.lock;           # defaults to root + "/Cargo.lock"
    bloomeryLock = null;                # auto-detected at root + "/bloomery.lock" if present
    members = null;                     # subset of crate names (null = build all)
  };

  # ── Toolchain & Linker ────────────────────────────────────────────────────
  toolchain = {
    rustc = pkgs.rustc;
    clippy = pkgs.clippy;
    cargo = pkgs.cargo;
    linker = "lld";                     # "lld" (default on Linux), "mold", or null (system)
  };

  # ── Compilation Profile ───────────────────────────────────────────────────
  profile = {
    optLevel = 3;                       # 0, 1, 2, 3, "s", "z"
    lto = "thin";                       # "fat", "thin", "off", or bool
    codegenUnits = 1;                   # positive int
    panic = "abort";                    # "unwind", "abort"
    strip = true;                       # true, false, "debuginfo", "symbols"
    targetCpu = null;                   # e.g. "x86-64-v3"
  };

  # ── Custom Compiler & Runner Flags ────────────────────────────────────────
  flags = {
    rustc = [ "-Copt-level=3" ];        # base flags for binary & library builds
    test = [];                          # extra flags for test runners
    clippy = [];                        # extra flags for clippy-driver
    doc = [ "-Dwarnings" ];             # extra flags for rustdoc
    doctest = [];                       # extra flags for doctest runner
  };

  # ── Crate Overrides (Native Dependencies) ─────────────────────────────────
  overrides = {
    openssl-sys = {
      nativeBuildInputs = [ pkgs.pkg-config ];
      buildInputs = [ pkgs.openssl ];
      rustcFlags = [];
      env = {};
      features = null;
    };
  };
}
```

### Colocated Package Overrides (`overrides.nix`)

In addition to top-level `overrides`, Bloomery automatically discovers and loads an `overrides.nix` file placed next to any workspace member's `Cargo.toml`. This allows individual packages to define custom filesets, compilation flags, environment variables, or native dependencies:

```nix
# crates/my-app/overrides.nix
{ pkgs, lib, ... }: {
  # Custom fileset (automatically converted to a clean derivation source)
  fileset = lib.fileset.unions [
    ./src
    ./Cargo.toml
    ./templates
    ./assets
  ];

  # Package-specific rustc compiler flags
  rustcFlags = [
    "-Ctarget-cpu=native"
  ];

  # Additional build-time tools and runtime C libraries
  nativeBuildInputs = [ pkgs.pkg-config ];
  buildInputs = [ pkgs.openssl ];

  # Environment variables for compilation
  env = {
    CUSTOM_ASSETS = "./assets";
  };
}
```

  # ── Development Shell (Direnv / nix develop) ──────────────────────────────
  devShell = {
    enable = true;                      # automatically generated devShell
    packages = [ pkgs.rust-analyzer ];  # extra shell packages
    shellHook = "";                     # bash setup script
  };

  # ── Checks & CI ───────────────────────────────────────────────────────────
  checks = {
    enable = true;                      # generate unit tests, clippy, doc, doctest checks
    includePackageChecks = true;        # include binary and library package builds in checks
    throwOnOutOfDate = false;           # fail evaluation immediately if lockfile is stale
  };

  # ── Feature Resolution ────────────────────────────────────────────────────
  features = {
    unify = true;                       # match Cargo's workspace feature unification
    cratesIoIndex = null;               # custom crates.io index directory
  };
}
```

---

## Workspace Outputs

Calling `mkWorkspace` returns an attribute set with:

- `packages`: Derivations for workspace member binaries, plus optional library and dev outputs.
- `apps`: Runnable app specifications for binaries, optional dev binaries, documentation, and `lock`.
- `checks`: Independent CI check derivations (`crate:test`, `crate:clippy`, `crate:doc`, `crate:doctest`, `crate:bin`, `crate:lib`, `workspace:lock`).
- `devShell`: Preconfigured development shell with rustc, clippy, cargo, nix-fast-build, and lld.
- `crates`: DAG attribute set of all built `.rlib` crates.
- `lock`: Parsed lockfile representation.
- `config`: Evaluated and type-checked options.

---

## Test Workspaces (Setup Styles)

Every supported flake setup style is verified by an isolated sub-flake in CI:

| Workspace | Setup Style | Description |
|---|---|---|
| [`tests/basic-workspace`](./tests/basic-workspace) | Zero-Boilerplate (`bloomery.mkFlake`) | Common five-member workspace layout with `build.rs`, unit tests, doctests, and crates.io dependencies |
| [`tests/axum-workspace`](./tests/axum-workspace) | Standard (`bloomery.lib.${system}.mkWorkspace`) | Axum webserver and CLI, including server asset packaging |
| [`tests/edge-cases-workspace`](./tests/edge-cases-workspace) | Zero-Boilerplate (`bloomery.mkFlake`) | Root package, glob members, excluded crates, build cfgs, integration tests, and multiple binary layouts |
| [`tests/mklib-workspace`](./tests/mklib-workspace) | Constructor (`bloomery.mkLib pkgs`) | Common workspace layout through a custom `pkgs` instance and compiler profile |
| [`tests/flake-parts-workspace`](./tests/flake-parts-workspace) | Flake-Parts Module (`bloomery.flakeModules.default`) | Common workspace layout through declarative `perSystem.bloomery.workspace` configuration |
| [`tests/overrides-workspace`](./tests/overrides-workspace) | Multi-Crate Overrides | Validates colocated member overrides and flake-level overrides merging in a multi-crate workspace |
| [`tests/single-crate-workspace`](./tests/single-crate-workspace) | Single-Crate Overrides | Validates root-package discovery and both colocated and flake-level overrides in a single-crate workspace |
