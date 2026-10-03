# Flags and option values

The command-specific options are:

| Command | Option | Accepted values | Default |
| --- | --- | --- | --- |
| `check` | none | — | — |
| `review` | `--format` | `text`, `json` | `text` |
| `sync` (planned) | `--update[=LIST]` | optional comma-separated `nix`, `rust` list | no upgrades; reconcile Cargo and Bloomery locks |
| all commands | `-h`, `--help` | — | — |

The format option controls only the rendering of the review catalog. Bloomery
does not expose a command-specific repository-root flag; check and review use
the current working directory, as does sync. The CLI parser also supplies the
`help` subcommand for displaying command help.

For sync, bare `--update` updates Rust dependencies and any existing Nix flake.
An explicit list updates only those ecosystems; explicit `nix` requires
`flake.nix`. Both `--update=nix,rust` and `--update nix,rust` are accepted.
Unknown values, empty elements, an explicit empty list, and repeated flags are
usage errors; duplicate names are deduplicated. Every successful sync still
reconciles Cargo.lock and regenerates bloomery.lock. See the
[sync selection contract](../../SYNC/design/sync.md#invocation-and-update-selection).
