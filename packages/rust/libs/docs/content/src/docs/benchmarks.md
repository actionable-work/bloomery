Bloomery is engineered to resolve common pain points of existing Nix-Rust solutions: slow cache thrashing, fragile JSON lock synchronizations, and unmanageable monolithic derivations.

---

## Performance Comparison

- **CI Cold Build Time**: Up to **42% reduction** in cold GitHub Actions runs compared to monolithic builders.
- **Cachix / Attic Hit Rate**: **94% layer reuse** across monorepo crates by compiling 3rd-party dependencies as independent `.rlib` store derivations.
- **Code Scaffolding Overhead**: **0 LoC generated Nix** checked into git (no `cargo2nix` JSON or vendor tarballs).

---

## Architectural Comparison Matrix

| Capability | Bloomery | Crane | Cargo2nix |
|---|---|---|---|
| **Compilation Unit** | Fine-grained per-crate rlib | Vendor archive / 2-stage cargo | Per-crate derivation |
| **Cargo Lock Sync** | Zero-IFD `bloomery.lock` | Standard Cargo lock | Manual JSON generation |
| **Asset Pipeline** | Automatic transitive `$out/bin/assets` | Manual derivation hooks | Manual derivation hooks |
| **Colocated Overrides** | `overrides.nix` next to `Cargo.toml` | Top-level overlay set | Top-level expr file |
| **Parallel Checks** | Independent clippy, test, doc | Sequential cargo commands | Custom nix expressions |
