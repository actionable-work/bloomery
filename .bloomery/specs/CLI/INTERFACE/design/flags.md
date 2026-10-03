# Flags and option values

The command-specific options are:

| Command | Option | Accepted values | Default |
| --- | --- | --- | --- |
| `check` | none | — | — |
| `review` | `--format` | `text`, `json` | `text` |
| all commands | `-h`, `--help` | — | — |

The format option controls only the rendering of the review catalog. Bloomery
does not expose a command-specific repository-root flag; check and review use
the current working directory. The CLI parser also supplies the `help`
subcommand for displaying command help.
