Bloomery maps Cargo semantics into a native Nix derivation graph. Understanding its architecture clarifies how hermetic builds and zero-IFD locking achieve unmatched speed.

---

## Zero-IFD Feature Resolution

In traditional Nix-Rust tools, determining which optional features and dependencies are active often triggers an Import-From-Derivation (IFD), forcing Nix evaluation to pause and launch a builder process:

1. The Bloomery CLI provides `bloomery sync`, which reconciles `Cargo.lock`, queries Cargo's resolved metadata, and produces `bloomery.lock`. Obtain the CLI with `nix run github:actionable-work/bloomery#bloomery -- sync` or add the Bloomery package to your development environment.
2. `bloomery.lock` captures:
   - Activated features per crate
   - Exact resolved runtime dependencies
   - Proc-macro flags and Rust editions
   - SHA-256 digest of `Cargo.lock`
3. During evaluation, `mkFlake` reads `bloomery.lock` via pure `builtins.fromTOML` with zero IFD. Lock validation is read-only; repair it by rerunning `bloomery sync`.

---

## Per-Crate Derivation Graph

Every crate in `Cargo.lock` is compiled by `builderCrate` into a dedicated Nix store derivation:

```
crates.io tarball (.crate)
         │
         ▼
  [builderCrate] ──> /nix/store/...-crate-0.1.0/lib/libcrate.rlib
         │                                       /nix/support/meta.sh
         ▼
  [builderBin]   ──> /nix/store/...-binary-0.1.0/bin/binary
                                                 /nix/store/...-binary-0.1.0/bin/assets/
```

- **Metadata Propagation**: Each compiled crate emits a `nix-support/meta.sh` script containing its crate name, library archive path, and sys-crate `DEP_<LINKS>_<KEY>` variables.
- **Transitive Closure**: Crates populate a lightweight `deps-closure/` directory with symlinks, allowing `rustc` to resolve indirect dependencies and proc-macro helpers without full tree re-scanning.

---

## Modern Build Script Directives

Bloomery compiles and executes build scripts in isolated sandbox environments, extracting both legacy (`cargo:`) and modern (`cargo::`) compiler directives:

- `cargo::rustc-cfg=...` (propagated to rustc `--cfg`)
- `cargo::rustc-link-lib=...` (propagated to rustc `-l`)
- `cargo::rustc-link-search=...` (propagated to rustc `-L`)
- `cargo::rustc-env=...` (exported to compilation environment)
- `cargo::rustc-flags=...` (parsed into compiler arguments)
- Sys-crate `DEP_<LINKS>_<KEY>` (propagated to dependent build scripts)

---

## Native Framework Asset Aggregation

Modern Rust GUI, game, and web frameworks (such as Topcoat, Dioxus, Bevy, or Leptos) often package static assets like CSS, images, shaders, or client bundles directly inside library crates. Traditional Nix builders drop these assets when linking binary derivations unless complex custom derivation hooks are manually authored.

Bloomery solves this natively inside `builderCrate` and `builderBin`:

1. **Automatic Library Export**: When a library crate contains `assets/`, `static/`, or `public/` directories, `builderCrate` installs them into the crate derivation and sets `DEP_HAS_ASSETS=1` in `nix-support/meta.sh`.
2. **Binary Aggregation**: When compiling a binary, `builderBin` automatically scans its direct and transitive dependency closure. Any assets discovered in upstream libraries are copied into `$out/bin/assets/` and `$out/share/<binName>/assets/`.
3. **Workspace & Custom Overrides**: Workspace-level assets and crate-specific directories (`assetDirs = [ "templates" ];` or `assets = [ ./data.txt ];`) are seamlessly bundled into the final executable package.
4. **Zero-Configuration Serving**: Binary executables can find their bundled assets alongside their executable path (`std::env::current_exe().parent().join("assets")`) in development and production alike.
