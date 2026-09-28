# The Bloomery Philosophy

> [!NOTE]
> A **bloomery** was the earliest metallurgical furnace invented by ancient smiths to smelt iron directly from ore and charcoal. Instead of producing brittle pig iron, it yielded a porous iron mass—a *bloom*—which was forged on the anvil to produce pure wrought iron.

In that same artisan spirit, **Bloomery** takes raw Rust crates and `Cargo.lock` manifests and reduces them directly into pure, hardened Nix store derivations—without invoking Cargo during the build.

---

## Why Pure Nix Derivations?

Traditional Nix-Rust builders (such as `naersk`, `crane`, or `buildRustPackage`) often wrap Cargo commands inside a single monolithic derivation or two-stage vendor archive:

- **Monolithic Invalidation**: Changing one line in a leaf binary or test forces re-evaluation and rebuild of large chunks of the dependency tree.
- **Cargo Lock Contention**: Running clippy, test, doc, and build in parallel frequently trips over Cargo's internal file locks on target directories.
- **Import From Derivation (IFD)**: Many tools run Cargo inside a throwaway derivation to inspect metadata, introducing costly evaluation overhead.

Bloomery eliminates these bottlenecks by operating directly at the compiler level:

1. **Every Crate is an Independent Derivation**: Dependencies are linked as exact `.rlib` archives via `rustc -L` and `--extern`.
2. **Zero Cargo Runtime Overhead**: Once resolved, each derivation runs `rustc` directly with precise compilation flags.
3. **True Incremental Caching**: Editing your binary crate only rebuilds that single binary. All library dependencies remain 100% untouched in `/nix/store`.
4. **Independent Parallel Checks**: Tests, clippy checks, rustdocs, and doctests run in parallel derivations without mutual locks.
5. **Hermetic & Reproducible**: No ambient network access during compilation; dependencies are securely hashed and tracked.

---

## Core Capabilities

- **`mkFlake`**: Zero-boilerplate flake generator creating packages, runnable apps, check suites, and devShells.
- **`mkWorkspace`**: Granular workspace builder supporting multi-crate workspaces, custom toolchains, and categorized options.
- **Strongly-Typed Profiles**: Evaluate Rust optimization flags (`optLevel`, `lto`, `codegenUnits`, `panic`, `strip`) through Nix type modules.
- **Colocated `overrides.nix`**: Per-crate fileset filtering, system libraries, and compiler flags located right alongside each `Cargo.toml`.
- **Zero-IFD `bloomery.lock`**: Deterministic feature resolution generated with `nix run .#lock`.
