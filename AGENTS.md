# Bloomery agent guide

Bloomery is a public Nix flake that builds Rust workspaces with `rustc` directly
from `Cargo.lock`, plus the `bloomery` CLI that syncs locks, runs checks, and
manages specifications. Other projects depend on it.

## Repository map

- `lib/`: the Nix library (`mkFlake`, `mkWorkspace`, crate graph, builders,
  profiles, options, config loader). Colocated `*.test.nix` unit tests are
  aggregated by `lib/tests.nix`.
- `nix/checks/`: flake checks for this repository.
- `packages/rust/bins/`: the `bloomery` CLI, docs site, docs checker, and
  Topcoat asset bundler. `packages/rust/libs/` holds their crates, grouped as
  `cli/*`, `shared/*`, and `docs/*`.
- `packages/playwright/`: Playwright suites.
- `templates/`: `bloomery init` templates. `tests/template-*` are locked
  fixtures built from them.
- `tests/`: fixture workspaces used by Nix tests and checks.
- `benchmarks/`: benchmark fixtures and reports.
- `.bloomery/specs/`: design documents and requirements, grouped by area (`CLI`,
  `DOCS`, `NIX`, `NIXLIB`, `PARSER`, `REPOSITORY`).
- `.bloomery/config.toml`: this repository's own Bloomery configuration.

## Compatibility

- Bloomery has external users. Prefer ergonomic interfaces, but give every
  breaking change a migration path or an actionable error.
- Renaming, moving, or changing the meaning or representation of an existing
  `.bloomery/config.toml` key requires a config format version bump
  (`CURRENT_CONFIG_VERSION` in `packages/rust/libs/shared/model/src/config.rs`
  and `currentConfigVersion` in `lib/build-config.nix`), a migration step in
  `packages/rust/libs/cli/config/src/upgrade.rs`, and updated version specs.
  Update every `version =` in templates, fixtures, docs, and test strings.
- Adding a key needs no format bump. Add it to the schema catalog
  (`packages/rust/libs/shared/schema/src/catalog.rs`), bump `CATALOG_VERSION`,
  and add recommended keys to every template config.
- This flake is public. Never add tests, fixtures, or docs derived from private
  code. Private projects may be used only for local profiling.

## Workflow

- We use jujutsu with conventional commits.
- Do not create commits or push to git remotes; I do that manually.
- Other agents may work in this repository at the same time. Use a separate jj
  workspace for parallel work. Stop only your own long-running commands; never
  `pkill` shared tools.
- Run `bloomery sync` after changing dependencies or crate layout.
- Confirm with me before adding a third-party dependency. Splitting internal
  crates is always allowed.

## Testing

- `bloomery check` is the main test entry point. When it is green, the workspace
  is ready to merge.
- Iterate with the dev outputs: `nix run .#bloomery:dev -- check`. Select checks
  with `--check <id-or-glob>` and list them with `check list`.
- Inspect failures with `check details <failure-id> --run <run-id>`, using the
  IDs the run prints.
- Don't edit files while a check run is in progress; stop it and rerun.
- The machine is often shared and heavily loaded, so wall-clock timings are
  noisy. Compare builds side by side or on an idle machine before drawing
  performance conclusions.

## Specifications

- Reflect every change in the `.bloomery/specs` design and requirement files.
- Prefer the CLI for spec edits (`nix run .#bloomery:dev -- spec add`,
  `spec set`, `spec remove`).
- Design documents capture the desired state only, in terse prose. Never
  describe the current state, the migration path to the desired state, or a lack
  of implementation evidence.
- We follow red-green spec design, so `MissingAutomatedTest` errors are expected
  at the end of a design phase.
- Each requirement covers one story. Prefer several similar requirements over
  one that validates several things.
- EARS types are `ubiquitous`, `event` (`trigger`), `state` (`state`),
  `unwanted_behavior` (`trigger`), `optional`, and `complex`.
- Mark a requirement manual only when it cannot possibly be tested
  automatically. Keep the manual list as small as possible.
- Tie tests to requirements with literal IDs: `#[cfg_attr(any(),
  bloomery("ID"))]` in Rust, `passthru.bloomery = [ ... ]` on Nix checks, and
  `tag: ['@bloomery:ID']` in Playwright. A test may carry at most 5
  requirements, and a requirement at most 5 tests. Split tests rather than
  adding `testExceptions` in `.bloomery/config.toml`.

## Code

- Favour readability and maintainability over clever code.
- Prefer writing our own code over pulling in a small dependency.
