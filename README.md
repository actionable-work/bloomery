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
  - Links rlib dependencies and workspace binaries using `rustc --crate-type bin`.
- **Tests without cargo**: Generates test runner executables using `rustc --test` and runs unit and integration tests.
- **Clippy without cargo**: Direct integration with `clippy-driver` to lint crates and enforce `-Dwarnings` in Nix derivations.

## Quickstart

1. In your Rust workspace, generate the bloomery lock manifest:
   ```bash
   nix run github:actionable/bloomery#lock
   ```
   This creates a `bloomery.lock` file next to your `Cargo.lock`.

2. Use bloomery in your `flake.nix`:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    bloomery.url = "github:actionable/bloomery";
  };

  outputs = { self, nixpkgs, bloomery }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};

      # Instantiate bloomery for your system
      bl = bloomery.mkLib.${system};

      # Compile your workspace
      workspace = bl.mkWorkspace {
        root = ./.;
        # Optional overrides for native C dependencies:
        overrides = {
          openssl-sys = {
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.openssl ];
          };
        };
      };
    in {
      # Workspace binaries (e.g. packages.x86_64-linux.my-binary)
      packages.${system} = workspace.packages // {
        default = workspace.packages.my-binary;
      };

      # Unit tests and Clippy linting
      checks.${system} = workspace.checks;

      # Lockfile generator app
      apps.${system} = {
        lock = bloomery.apps.${system}.lock;
      };
    };
}
```

## Structure

- [`lib/builders/`](./lib/builders): Modular compilers for libraries, binaries, test runners, docs, and clippy using raw `rustc`.
- [`lib/workspace/`](./lib/workspace): Parses `Cargo.lock`, discovers workspace crates, and resolves profiles.
- [`lib/lock/`](./lib/lock): Generates `bloomery.lock` manifests via `cargo metadata`.
- [`lib/overrides/`](./lib/overrides): Built-in native dependencies for common `-sys` crates (`openssl`, `libpq`, `zlib`, `sqlite3`, etc.).
- [`tests/test-workspace`](./tests/test-workspace): Complete reference workspace with library, binary, `build.rs`, unit tests, and crates.io dependencies.
