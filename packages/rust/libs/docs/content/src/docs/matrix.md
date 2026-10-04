Bloomery verifies its own correctness against dedicated test workspaces, running automated checks in CI:

---

## Workspace Integration Matrix

| Test Workspace | Integration Style | Description & Tested Levers |
|---|---|---|
| `tests/basic-workspace` | **Config-driven `mkFlake`** | Evaluates `bloomery.mkFlake` with a `.bloomery/config.toml`, testing multi-crate workspace compilation, libs, binaries, and tests. |
| `tests/axum-workspace` | **Config-driven `mkFlake`** | Full real-world Axum web server and Clap CLI with linker settings and profiles from `config.toml`. |
| `tests/single-crate-workspace` | **Nix `overrides` argument** | Exercises the `overrides` argument accepted by `mkFlake`. |
| `tests/flake-parts-workspace` | **Internal flake-parts module** | Exercises the internal flake-parts module directly; the public interface is `mkFlake`. |
| `tests/overrides-workspace` | **Overrides Validation** | Validates colocated `overrides.nix` files (fileset unions, compilation flags, env) and override merging. |
| `tests/edge-cases-workspace` | **Edge Cases Validation** | Validates directory tests, multi-file binaries, weak feature syntax, and modern `cargo::` directives. |
