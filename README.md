# bloomery

A pure Nix library for building Rust applications and workspaces directly from `Cargo.lock` and `rustc`, completely bypassing `cargo`.

Inspired by `oxalica/nocargo`, this flake:
- **Direct `Cargo.lock` parsing**: Reads package versions and dependency trees purely in Nix using `builtins.fromTOML` (no external code generation).
- **Direct source fetching with zero IFD**: Downloads crates.io dependencies via `pkgs.fetchurl` using the SHA-256 checksums already recorded in `Cargo.lock`.
- **Pure `rustc` compiler driver**:
  - Compiles `.rlib` dependencies and `proc-macro` crates independently.
  - Automatically compiles and executes build scripts (`build.rs`), capturing generated environment variables, `cargo:rustc-cfg`, and native library link search paths.
  - Links rlib dependencies and workspace binaries using `rustc --crate-type bin`.
- **Tests without cargo**: Generates test runner executables using `rustc --test` and runs unit and integration tests.
- **Clippy without cargo**: Direct integration with `clippy-driver` to lint crates and enforce `-Dwarnings` in Nix derivations.

## Usage

### In your `flake.nix`

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    bloomery.url = "path:./bloomery"; # or github:your-repo/bloomery
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
    };
}
```

## Structure

- [`lib/parse-lock.nix`](./lib/parse-lock.nix): Parses `Cargo.lock` into a dependency DAG.
- [`lib/build-crate.nix`](./lib/build-crate.nix): Compiles an individual `.rlib` or `proc-macro` using `rustc`.
- [`lib/build-bin.nix`](./lib/build-bin.nix): Compiles final workspace executables.
- [`lib/test-crate.nix`](./lib/test-crate.nix): Compiles and runs test runners with `rustc --test`.
- [`lib/clippy-crate.nix`](./lib/clippy-crate.nix): Lints code with `clippy-driver`.
- [`lib/default-overrides.nix`](./lib/default-overrides.nix): Built-in native dependencies for common `-sys` crates (`openssl`, `libpq`, `zlib`, `sqlite3`, etc.).
- [`tests/test-workspace`](./tests/test-workspace): Complete reference workspace with library, binary, `build.rs`, unit tests, and crates.io dependencies.
