# Quickstart Guide

Get up and running with Bloomery in less than 60 seconds.

---

## 1. Zero-Boilerplate with `mkFlake`

For standard workspaces, `bloomery.mkFlake` sets up complete packages, runnable apps, check suites, and development shells with just a few lines of Nix:

```nix
# flake.nix
{
  description = "My Rust Application";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    bloomery.url = "github:actionable/bloomery";
  };

  outputs = { self, nixpkgs, bloomery, ... }:
    bloomery.mkFlake {
      inherit self nixpkgs;
      root = ./.;
    };
}
```

> [!TIP]
> `mkFlake` automatically searches default systems (`x86_64-linux`, `aarch64-linux`, `aarch64-darwin`) and exports standard outputs:
> - `packages.${system}.<name>` and `packages.${system}.default`
> - `apps.${system}.<name>` and `apps.${system}.default`
> - `checks.${system}.<name>:<test|clippy|doc|doctest>`
> - `devShells.${system}.default` (with `rustc`, `clippy`, `cargo`, and fast-build tools)

---

## 2. Generating the Lock Manifest

Bloomery resolves all crate dependencies and features without IFD. Run the lock updater once to initialize `bloomery.lock`:

```bash
nix run github:actionable/bloomery#lock
```

Commit `bloomery.lock` to your repository:

```bash
git add bloomery.lock && git commit -m "chore: add bloomery lockfile"
```

---

## 3. Running & Checking

Build and run your default binary:

```bash
nix run .
```

Run all unit tests, clippy checks, and documentation builders in parallel:

```bash
nix flake check
```

Or run fast parallel builds with `nix-fast-build`:

```bash
nix develop --command nix-fast-build --no-link --skip-cached
```
