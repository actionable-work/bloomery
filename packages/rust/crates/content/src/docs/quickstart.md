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
> - `packages.${system}.<name>`, `packages.${system}.<name>:dev`, and `packages.${system}.default`
> - `apps.${system}.<name>`, `apps.${system}.<name>:dev`, and `apps.${system}.default`
> - `checks.${system}.<name>:<test|clippy|doc|doctest>` and `checks.${system}.workspace:lock`
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

## Generating the Lock Manifest

Bloomery resolves all crate dependencies and features deterministically without Import-From-Derivation (IFD). Run the lock generator once to initialize `bloomery.lock`:

```bash
nix run github:actionable-work/bloomery#lock
```

Or run the lock generator locally:

```bash
nix run .#lock
```

Commit `bloomery.lock` to version control:

```bash
git add bloomery.lock && git commit -m "chore: add bloomery lockfile"
```

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

