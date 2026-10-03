Bloomery verifies its own correctness against 6 dedicated test workspaces, running 68+ automated checks in CI:

---

## Workspace Integration Matrix

| Test Workspace | Integration Style | Description & Tested Levers |
|---|---|---|
| `tests/basic-workspace` | **Style 1: Zero-Boilerplate** | Evaluates `bloomery.mkFlake` testing multi-crate workspace compilation, libs, binaries, and tests. |
| `tests/axum-workspace` | **Style 2: Categorized `mkWorkspace`** | Full real-world Axum web server and Clap CLI with mold/lld linker settings and profiles. |
| `tests/mklib-workspace` | **Style 3: Custom Lib Constructor** | Constructor `bloomery.mkLib pkgs` with custom Nixpkgs instances, overlays, and system overrides. |
| `tests/flake-parts-workspace` | **Style 4: Flake-Parts Module** | Clean integration into flake-parts modules using `bloomery.flakeModules.default`. |
| `tests/overrides-workspace` | **Overrides Validation** | Validates colocated `overrides.nix` files (fileset unions, compilation flags, env) and override merging. |
| `tests/edge-cases-workspace` | **Edge Cases Validation** | Validates directory tests, multi-file binaries, weak feature syntax, and modern `cargo::` directives. |
