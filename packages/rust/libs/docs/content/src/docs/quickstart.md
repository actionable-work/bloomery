## Zero-Boilerplate with `mkFlake`

For standard workspaces, `bloomery.mkFlake` sets up complete packages, runnable apps, check suites, and development shells with just a few lines of Nix:

```nix
# flake.nix
{
  description = "My Rust Application";

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

> [!TIP]
> `mkFlake` automatically searches default systems (`x86_64-linux`, `aarch64-linux`, `aarch64-darwin`) and exports standard flake attributes:
> - `packages.${system}.<name>`, optional `packages.${system}.<crate>:lib`, and `packages.${system}.default` when a binary exists
> - `apps.${system}.<name>`, `apps.${system}.<name>:dev`, `apps.${system}.<crate>:doc`, and `apps.${system}.default` when a binary exists
> - `checks.${system}.<name>:<test|clippy|doc|doctest>` and `checks.${system}.workspace:lock`; run `bloomery check` directly to combine static specification validation and selected Nix checks.
> - `devShells.${system}.default` (with `rustc`, `clippy`, `cargo`, and fast-build tools)

---

## Alternative Integration Styles

### 1. Categorized Workspace (`mkWorkspace`)

When building inside an existing per-system function or custom flake layout, call `bloomery.lib.${system}.mkWorkspace`:

```nix
{
  outputs = { nixpkgs, bloomery, ... }: let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
    ws = bloomery.lib.${system}.mkWorkspace {
      root = ./.;
      profile = { optLevel = 3; lto = "thin"; };
    };
  in {
    packages.${system} = ws.packages;
    checks.${system} = ws.checks;
    devShells.${system}.default = ws.devShell;
  };
}
```

### 2. Flake-Parts Integration (`flakeModules.default`)

Integrate Bloomery cleanly into `flake-parts` modules:

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
      perSystem = { pkgs, ... }: {
        bloomery.workspace = {
          root = ./.;
        };
      };
    };
}
```

### 3. Custom Lib Constructor (`bloomery.mkLib`)

If you pass customized Nixpkgs instances, overlays, or stdenv cross-compilers:

```nix
let
  bl = bloomery.mkLib pkgs;
  ws = bl.mkWorkspace { root = ./.; };
in ws.packages.my-bin
```

---

## Synchronizing Lockfiles

Bloomery resolves crate features and dependency edges without Import-From-Derivation (IFD). Use the Bloomery CLI package to reconcile Cargo manifests and generate `bloomery.lock`:

```bash
nix run github:actionable-work/bloomery#bloomery -- sync
```

This runs Cargo's normal workspace resolution, preserving compatible locked versions where possible. Request upgrades explicitly with `--update=rust`, `--update=nix`, or bare `--update` to update all applicable ecosystems. Sync requires `cargo`; it requires `nix` only when updating a flake. It does not require a specification tree or create configuration files.

The `check`, `review`, and `sync` commands all accept the same `--json` flag for machine-readable output. Human-readable output is colorized when writing to a terminal and respects `NO_COLOR`.

Run `nix run .#bloomery:dev -- check` to validate specifications and execute selected Nix checks. Check retains results and logs in a private workspace-scoped cache outside the repository; logs may contain sensitive build output and remain until you delete the cache. Use `check list`, `check failures`, and `check details` to discover checks and page through retained results without rerunning them.

Commit the generated `Cargo.lock` (if newly created), `bloomery.lock`, and any selected `flake.lock` update to version control.

---

## Running and Checking

Build and run your default binary executable:

```bash
nix run .
```

Run unit tests, clippy driver checks, rustdoc builders, and lock validation in parallel derivations:

```bash
nix flake check
```

Or drop into an isolated shell with preconfigured `rustc`, `cargo`, and dev tools:

```bash
nix develop
```
