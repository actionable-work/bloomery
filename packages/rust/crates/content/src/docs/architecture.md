# Bloomery Architecture & Internals

Bloomery maps Cargo semantics into a native Nix derivation graph. Understanding its architecture clarifies how hermetic builds and zero-IFD locking achieve unmatched speed.

---

## 1. Zero-IFD Feature Resolution

In traditional Nix-Rust tools, determining which optional features and dependencies are active often triggers an Import-From-Derivation (IFD), forcing Nix evaluation to pause and launch a builder process:

1. Bloomery provides the `lock` tool (`nix run .#lock`), which queries `cargo metadata --format-version 1` and produces `bloomery.lock`.
2. `bloomery.lock` captures:
   - Activated features per crate
   - Exact resolved runtime dependencies
   - Proc-macro flags and Rust editions
   - SHA-256 digest of `Cargo.lock`
3. During evaluation, `mkWorkspace` reads `bloomery.lock` via pure `builtins.fromTOML` with zero IFD.

---

## 2. Derivation Per Crate

Every crate in `Cargo.lock` is compiled by `builderCrate` into a dedicated Nix store derivation:

```
crates.io tarball (.crate)
         │
         ▼
  [builderCrate] ──> /nix/store/...-crate-0.1.0/lib/libcrate.rlib
         │                                       /nix/support/meta.sh
         ▼
  [builderBin]   ──> /nix/store/...-binary-0.1.0/bin/binary
```

- **Metadata Propagation**: Each compiled crate emits a `nix-support/meta.sh` script containing its crate name, library archive path, and sys-crate `DEP_<LINKS>_<KEY>` variables.
- **Transitive Closure**: Crates populate a lightweight `deps-closure/` directory with symlinks, allowing `rustc` to resolve indirect dependencies and proc-macro helpers without full tree re-scanning.

---

## 3. Modern Build Script (`build.rs`) Directives

Bloomery compiles and executes build scripts in isolated sandbox environments, extracting both legacy (`cargo:`) and modern (`cargo::`) compiler directives:

- `cargo::rustc-cfg=...` (propagated to rustc `--cfg`)
- `cargo::rustc-link-lib=...` (propagated to rustc `-l`)
- `cargo::rustc-link-search=...` (propagated to rustc `-L`)
- `cargo::rustc-env=...` (exported to compilation environment)
- `cargo::rustc-flags=...` (parsed into compiler arguments)
- Sys-crate `DEP_<LINKS>_<KEY>` (propagated to dependent build scripts)
