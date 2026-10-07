# Commands and invocation

The `bloomery` executable exposes the application commands and the CLI parser's
built-in help command:

```text
bloomery init [DIRECTORY] [--template NAME] [--force] [--json]
bloomery check [--check ID_OR_GLOB]... [--system SYSTEM]... [--jobs N] [--fail-fast] [--json]
bloomery check list [--check ID_OR_GLOB]... [--system SYSTEM]... [--offset N] [--limit N] [--json]
bloomery check failures [--run RUN] [--offset N] [--limit N] [--json]
bloomery check details FAILURE [--run RUN] [--offset N] [--limit N] [--json]
bloomery review [--json]
bloomery sync [--json] [--update[=nix,rust]]
bloomery spec list [--area AREA] [--feature FEATURE] [--group GROUP] [--json]
bloomery spec show ID [--json]
bloomery spec add ID --title TITLE --ears TOML [--design PATH] [--manual] [--json]
bloomery spec set ID FIELD VALUE [--json]
bloomery spec remove ID [--json]
bloomery spec trace ID... [--json]
bloomery spec candidates [--json]
bloomery config get KEY [--json]
bloomery config set KEY VALUE [--json]
bloomery config unset KEY [--json]
bloomery config list [--prefix KEY] [--json]
bloomery config upgrade [--dry-run] [--json]
bloomery config document [--json]
bloomery help [COMMAND]
```

The `bloomery` binary delegates to `bloomery-cli-app`.
`bloomery-cli-parser` converts arguments into Clap-independent request types,
and the application library dispatches those requests to command libraries.

## Flake presence preflight

Before command handling, resolve the root from the process's current working
directory and require a `flake.nix` file there. This applies to `check` and its
nested commands, `review`, `sync`, and `spec`. There are three exemptions. The
workspace-bootstrap `init` command creates the flake and may run in a directory
without one. Parser-generated help and version output is returned without the
preflight. The `config` command edits `.bloomery/config.toml` without a flake
and emits an advisory warning naming the missing Bloomery flake instead of
failing.

If `flake.nix` is absent for a non-exempt command, stop before command-specific
work and emit a nonzero error with this message:

```text
A Bloomery flake.nix is required; set up a Bloomery flake.nix in the current directory before running this command.
```

With `--json`, emit one structured JSON setup error with the same message. The
preflight checks file presence only; a present flake is handled by each
command's own behavior. No upward workspace search is performed. For `config`,
the missing flake produces a structured warning rather than a setup error, and
the command proceeds. Help and version output is written without inspecting the
flake, so `bloomery --help`, `bloomery help COMMAND`, and `bloomery COMMAND
--help` all succeed where no flake is present.

## Configuration presence

After the flake preflight, require `.bloomery/config.toml` before command
handling for `check`, its nested commands, `review`, `sync`, and `spec`. Parser
help,
 the `init` command, and every `config` command are exempt. If the file is
 absent, non-config commands stop before command-specific work and emit a
 nonzero `ConfigurationError` naming the missing path. A mutating `config`
 command instead creates the `.bloomery/` directory and an empty configuration
 file, emits an advisory warning naming the created path, and continues; a
 read-only `config` command reports against defaults with the same warning
 without creating anything. With `--json`, non-config commands emit one
 structured error and `config` emits a structured warning. The specs and
 scanner tables are then loaded as described in
 [PARSER/CONFIGURATION](../../../PARSER/CONFIGURATION/design/config-file.md).

The `check`, `review`, `sync`, and `spec` commands operate on the validated
root. The
`init` command operates on its target directory instead and requires no
existing configuration. `config` creates configuration on demand and then
edits it, and its `document` subcommand annotates configured keys with the
schema-catalog documentation without changing values. Sync maintains lockfiles
without loading specifications or scanning evidence; config edits the
configuration without loading specifications or scanning evidence; spec applies
read-only record edits without scanning source, while its trace and candidate
commands scan configured source without writing. Command
semantics are specified by
[INIT](../../INIT/README.md), [CHECK](../../CHECK/README.md),
[REVIEW](../../REVIEW/README.md), [SYNC](../../SYNC/README.md),
[SPEC](../../SPEC/README.md), and
[CONFIG](../../CONFIG/README.md). Sync is a CLI-only replacement for the Nix
lock app, not a new Nix app. The global `--json` flag is available after any
application subcommand and their nested commands; see the
[output contract](output.md). Check without a nested command executes the
selected suite; list discovers checks, failures pages retained failures, and
details seeks within retained failure output. Retrieval never executes tests or
builds. Config commands are specified separately by [CONFIG](../../CONFIG/README.md).
