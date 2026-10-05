---
id: CONFIG
name: Typesafe Configuration Editing
tagline: Schema-typed CRUD, diff, and upgrade commands for .bloomery/config.toml.
description: |
  The config feature owns the `bloomery config` command tree. It reads and
  edits every key of `.bloomery/config.toml` through schema-typed dotted keys,
  provides `config upgrade` to materialize absent recommended keys and
  `config upgrade --diff` to compare the recommended configuration with the
  current one without changing anything, and provides `config document` to
  annotate configured keys with their schema-catalog documentation.
---

# `bloomery config`

`bloomery config` is the only CLI surface that mutates `.bloomery/config.toml`.
It exposes typed create, read, update, and delete operations over the
configuration schema catalog plus an upgrade command that fills in recommended
keys introduced by newer Bloomery releases and a document command that adds the
catalog documentation comments to configured keys.

The [schema catalog](../../PARSER/CONFIGURATION/design/schema-catalog.md) spans
the CLI-owned `[specs]`/`[scanners.*]` tables and the Nix build tables
`[build]`, `[toolchain]`, `[profile.*]`, `[flags]`, `[devShell]`, `[checks]`,
and `[features]`. The CLI validates and edits all of them;
[NIXLIB/FLAKE](../../NIXLIB/FLAKE/README.md) retains authority over build-table
semantics and evaluation-time resolution. Command syntax is owned by
[CLI/INTERFACE](../INTERFACE/README.md). Config editing does not load the
specification tree, scan evidence, run checks, or touch lockfiles.

## Design documents

- [Typesafe editing](design/editing.md)
- [Configuration upgrade](design/upgrade.md)
- [Configuration documentation](design/documenting.md)

## Responsibilities

- Expose `config get`, `config set`, `config unset`, `config list`,
  `config upgrade` with `--diff`, and `config document`.
- Address every catalogued configuration key by dotted path.
- Parse and validate supplied values against the schema type of the addressed
  key before writing.
- Preserve every unrelated key, comment, and formatting choice.
- Validate the complete resulting configuration and publish it atomically.
- Add only absent recommended keys during `config upgrade` and report
  recommendation differences with `--diff`.
- Annotate every configured key with its schema-catalog documentation during
  `config document` without changing values or materializing absent keys.
- Warn, without failing, when a config command runs outside a Bloomery flake
  or creates a missing configuration file.
- Report human-readable and JSON results with stable exit codes.

## Boundaries

Config addresses every recognized configuration table, including the Nix build
tables. It does not evaluate the flake, resolve nixpkgs attribute paths, run
checks, or regenerate locks. Mutating `config` commands create
`.bloomery/config.toml` when it is absent, warning while doing so; read-only
commands report defaults without writing. `config document` edits only the
comment layer, leaving values, present keys, and absent keys untouched.
`bloomery init` remains the template-driven workspace bootstrap. Unlike other
repository commands, config does not require a root `flake.nix`, but it warns
when one is absent.

## Verification

Requirements in this feature are automated (`manual = false`). Use temporary
workspace fixtures to verify type validation, upsert and delete semantics,
format preservation, atomic failure behavior, diff reporting, documentation
annotation and idempotence, and upgrade idempotence. Static inspection covers
CLI exposure and output contracts.
