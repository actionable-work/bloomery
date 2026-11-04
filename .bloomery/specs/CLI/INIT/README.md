---
id: INIT
name: Workspace Initialization
tagline: Scaffold a new Bloomery flake and Rust workspace from a bundled template.
description: |
  The init feature owns the bootstrap `bloomery init` command. It selects a
  bundled template, derives the project name, writes a multi-crate Bloomery
  flake using Bloomery's nixpkgs input, Rust workspace, configuration with all
  recommended defaults, specification skeleton, gitignore, and direnv files
  into a target directory, then locks the generated workspace.
---

# `bloomery init`

Init is the workspace bootstrap command. It is the only command exempt from the
shared root `flake.nix` and `.bloomery/config.toml` preflights, because it
creates those files. It writes source files, seeds a limited specification
skeleton, and then locks the generated workspace so it is immediately usable.

## Design documents

- [Initialization behavior](design/init.md)
- [Template catalog and content](design/templates.md)

## Responsibilities

- Expose the `init` subcommand with an optional target directory.
- Accept `--template` selection, defaulting to the basic workspace template.
- Accept `--force` to overwrite colliding template paths, and otherwise warn
  and exit before writing when a template path or an ancestor path conflicts.
- Resolve a template from Bloomery's bundled catalog named after each
  immediate child of the repository's `templates/` directory.
- Derive and validate the project name used by the `{{project}}` placeholder.
- Write the common scaffolding: `flake.nix`, a resolver-2 `Cargo.toml` with
  centralized dependencies, disabled default features, and member inheritance;
  `.bloomery/config.toml` with the current format version and every recommended
  configuration default; a seeded `.bloomery/specs` skeleton; `.gitignore`; and
  `.envrc`.
- Provide the `basic`, `axum`, and `topcoat` multi-crate templates with thin
  binaries over library crates.
- Lock the generated workspace by creating `Cargo.lock`, `bloomery.lock`, and
  `flake.lock` even when scaffold files are untracked in an enclosing Git
  worktree, without requiring direnv approval.
- Report the target, template, created paths, lockfiles, and suggested next
  commands in human and JSON output.

## Boundaries

Init owns filesystem scaffolding and the initial lock pass. It never builds
crates and does not overwrite an existing template path without `--force`.
Lock reconciliation reuses the [`bloomery sync`](../SYNC/README.md) workflow.
Seeded specification requirements are automated and backed by tagged unit tests
in the generated libraries. Dependency updates, spec authoring beyond the seed
skeleton, and project publication are outside this feature.

## Verification

All requirements are automated (`manual = false`). Scaffolding is verified by
running `init` against temporary target directories with injectable lock
runners and asserting the created paths, generated contents, template
selection, placeholder substitution, seeded specs, lockfiles, force/guard
behavior, and failure handling. Locking coverage includes targets inside Git
worktrees with untracked scaffold files and targets whose `.envrc` has not been
approved by direnv. The root flake builds each template workspace's generated
checks, and a source check verifies each workspace fixture matches its template
after project-name substitution.
