# Commands and invocation

The `bloomery` executable exposes the application commands and the CLI parser's
built-in help command:

```text
bloomery check [--json]
bloomery review [--json]
bloomery sync [--json] [--update[=nix,rust]]
bloomery help [COMMAND]
```

The parser also handles help requests such as `bloomery --help` and
`bloomery check --help`. The `check`, `review`, and `sync` commands operate on the repository rooted at
the process's current working directory; the help command does not load a
repository. Sync maintains lockfiles without loading specifications or scanning
evidence. Command semantics are specified by
[CHECK](../../CHECK/README.md), [REVIEW](../../REVIEW/README.md), and
[SYNC](../../SYNC/README.md). Sync is a CLI-only replacement for the Nix lock
app, not a new Nix app. The global `--json` flag is available after any of the
three application subcommands; see the [output contract](output.md).
