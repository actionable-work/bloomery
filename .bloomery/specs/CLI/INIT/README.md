---
id: INIT
name: Workspace Initialization
tagline: Scaffold a new Bloomery flake and Rust workspace from a bundled template.
description: |
  The init feature owns the bootstrap `bloomery init` command. It selects a
  bundled template, derives the project name, writes a multi-crate Bloomery
  flake, Rust workspace, configuration, specification skeleton, gitignore, and
  direnv files into a target directory using Bloomery defaults, then locks the
  generated workspace.
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
- Write the common scaffolding: `flake.nix`, `Cargo.toml`,
  `.bloomery/config.toml`, a seeded `.bloomery/specs` skeleton, `.gitignore`,
  and `.envrc`.
- Provide the `basic`, `axum`, and `topcoat` multi-crate templates with thin
  binaries over library crates.
- Lock the generated workspace by creating `Cargo.lock`, `bloomery.lock`, and
  `flake.lock`.
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
behavior, and failure handling.
