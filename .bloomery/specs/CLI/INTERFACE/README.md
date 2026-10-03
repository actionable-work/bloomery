---
id: INTERFACE
name: Bloomery CLI Interface
tagline: Define the user-visible command syntax and options for the bloomery executable.
description: |
  The interface feature specifies the command tree, invocation root, and
  command-specific flags exposed by the `bloomery` executable. Command behavior
  is specified separately by the CHECK and REVIEW features.
---

# CLI Interface

This feature is the source of truth for the syntax exposed by the `bloomery`
executable. It covers command names, option names, accepted values, defaults,
and how the target repository root is selected.

## Design documents

- [Commands and invocation](design/commands.md)
- [Flags and option values](design/flags.md)

## Responsibilities

- Expose the `check` and `review` subcommands and parser-generated help.
- Resolve the repository root from the process's current working directory for
  verification commands.
- Specify `review --format` values and its default.
- Keep command syntax distinct from check and review semantics.
