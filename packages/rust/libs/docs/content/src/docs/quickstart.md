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

> [!IMPORTANT]
> `mkFlake` requires `.bloomery/config.toml`. Create it next to `flake.nix`:
>
> ```toml
> [build]
> libPackages = false
> devPackages = true
>
> [profile.release]
> optLevel = 3
> lto = "thin"
> codegenUnits = 1
>
> [toolchain]
> linker = "lld"
>
> [checks]
> enable = true
> ```
>
> Every workspace setting lives in `config.toml`; `mkFlake` receives only
> `nixpkgs`, `root`, an optional `systems` list, an optional `overrides` set,
> and an optional `extraOutputs` callback.

---

## Configuration Tables

All build settings are read from `.bloomery/config.toml`:

| Table | Purpose |
| --- | --- |
| `[build]` | Source paths, member selection, `profileName`, `libPackages`, `devPackages`, `unify` |
| `[toolchain]` | `rustc`, `clippy`, `cargo`, `lld`, `mold`, `stdenv`, and `linker` |
| `[profile.release]`, `[profile.dev]` | Optimization, LTO, codegen units, panic, strip, debug info, CPU target |
| `[flags]` | Extra `rustc`, `test`, `clippy`, `doc`, and `doctest` flags |
| `[devShell]` | `enable`, extra `packages`, and `shellHook` |
| `[checks]` | `enable`, `includePackageChecks`, `throwOnOutOfDate` |
| `[features]` | `cratesIoIndex` |

Package-valued settings are written as nixpkgs attribute paths (for example
`linker = "mold"` or `packages = ["rust-analyzer"]`); path-valued settings are
relative to the workspace root. Per-crate overrides stay in Nix: pass them to
`mkFlake` through the `overrides` argument or place an `overrides.nix` next to a
crate's `Cargo.toml`.

---

## Synchronizing Lockfiles

Bloomery resolves crate features and dependency edges without Import-From-Derivation (IFD). Use the Bloomery CLI package to reconcile Cargo manifests and generate `bloomery.lock`:

```bash
nix run github:actionable-work/bloomery#bloomery-wrapped -- sync
```

This runs Cargo's normal workspace resolution, preserving compatible locked versions where possible. Request upgrades explicitly with `--update=rust`, `--update=nix`, or bare `--update` to update all applicable ecosystems. Sync requires `cargo`; it requires `nix` only when updating a flake. It requires `.bloomery/config.toml` and does not create or edit it.

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
