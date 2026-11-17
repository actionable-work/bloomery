Bloomery verifies its own correctness against dedicated test workspaces, running automated checks in CI:

The `docs:playwright` check runs the packaged docs server and Chromium inside a
Nix test derivation. Browser examples in `packages/playwright/docs` cover sidebar
navigation, search, keyboard dismissal and native assets. Runner integration
checks exercise readiness, reports, assertion failures, focused tests, empty
suites and server failures. Run them with `bloomery check`; select just the docs
suite with `bloomery check --check '*docs:playwright'`.

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
