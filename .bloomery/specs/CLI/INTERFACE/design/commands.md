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

The parser also handles help requests such as `bloomery --help` and
`bloomery check --help`. The `bloomery` binary delegates to `bloomery-cli-app`.
`bloomery-cli-parser` converts arguments into Clap-independent request types,
and the application library dispatches those requests to command libraries.
The `check`, `review`, and `sync` commands operate on the repository rooted at
the process's current working directory; the help command does not load a
repository. Sync maintains lockfiles without loading specifications or scanning
evidence. Command semantics are specified by
[CHECK](../../CHECK/README.md), [REVIEW](../../REVIEW/README.md), and
[SYNC](../../SYNC/README.md). Sync is a CLI-only replacement for the Nix lock
app, not a new Nix app. The global `--json` flag is available after any of the
three application subcommands and their nested commands; see the
[output contract](output.md). Check without a nested command executes the selected
suite; list discovers checks, failures pages retained failures, and details seeks
within retained failure output. Retrieval never executes tests or builds.
