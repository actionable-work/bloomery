# Commands and invocation

The `bloomery` executable exposes the application commands and the CLI parser's
built-in help command:

```text
bloomery check [--check ID_OR_GLOB]... [--system SYSTEM]... [--jobs N] [--fail-fast] [--json]
bloomery check list [--check ID_OR_GLOB]... [--system SYSTEM]... [--offset N] [--limit N] [--json]
bloomery check failures [--run RUN] [--offset N] [--limit N] [--json]
bloomery check details FAILURE [--run RUN] [--offset N] [--limit N] [--json]
bloomery review [--json]
bloomery sync [--json] [--update[=nix,rust]]
bloomery help [COMMAND]
```

The `bloomery` binary delegates to `bloomery-cli-app`.
`bloomery-cli-parser` converts arguments into Clap-independent request types,
and the application library dispatches those requests to command libraries.

## Flake presence preflight

Before command handling or parser-generated help is returned, resolve the root
from the process's current working directory and require a `flake.nix` file
there. This applies to `check` and its nested commands, `review`, `sync`, and
help requests. The sole exception is the workspace-bootstrap `init` command.

If `flake.nix` is absent, stop before command-specific work and emit a nonzero
error with this message:

```text
A Bloomery flake.nix is required; set up a Bloomery flake.nix in the current directory before running this command.
```

With `--json`, emit one structured JSON setup error with the same message. The
preflight checks file presence only; a present flake is handled by each
command's own behavior. No upward workspace search is performed.

The `check`, `review`, and `sync` commands operate on the validated root. Sync
maintains lockfiles without loading specifications or scanning evidence.
Command semantics are specified by [CHECK](../../CHECK/README.md),
[REVIEW](../../REVIEW/README.md), and [SYNC](../../SYNC/README.md). Sync is a
CLI-only replacement for the Nix lock app, not a new Nix app. The global
`--json` flag is available after any of the three application subcommands and
their nested commands; see the [output contract](output.md). Check without a
nested command executes the selected suite; list discovers checks, failures
pages retained failures, and details seeks within retained failure output.
Retrieval never executes tests or builds.
