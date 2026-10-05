---
id: INTERFACE
name: Bloomery CLI Interface
tagline: Define the user-visible command syntax and options for the bloomery executable.
description: |
  The interface feature specifies the command tree, invocation root, shared
  flake-presence preflight, flags, and output modes exposed by `bloomery`.
  Command behavior is specified separately by the CHECK, REVIEW, and SYNC features.
---

# CLI Interface

This feature is the source of truth for the syntax exposed by the `bloomery`
executable. It covers command names, option names, accepted values, defaults,
and how the target repository root is selected.

## Design documents

- [Commands and invocation](design/commands.md)
- [Flags and option values](design/flags.md)
- [Human and machine-readable output](design/output.md)

## Responsibilities

- Expose `init`, `check`, its `list`/`failures`/`details` commands, `review`,
  `sync`, and parser-generated help.
- Resolve the repository root from the process's current working directory.
- Require a root `flake.nix` before every command except the workspace-bootstrap
  `init` command.
- Require `.bloomery/config.toml` for repository commands while exempting parser
  help and `init`.
- Specify shared `--json`, check selection/execution/retrieval options, and
  command-specific `sync --update` and `init --template`/`init --force` values.
- Define human-readable terminal coloring and stable JSON output contracts.
- Keep command syntax distinct from init, check, review, and sync semantics.
