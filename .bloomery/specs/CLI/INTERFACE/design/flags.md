# Flags and option values

## Shared-purpose options

Shared-purpose flags use the same name and behavior across the application
commands:

| Command | Option | Accepted values | Default |
| --- | --- | --- | --- |
| `check` | `--json` | flag | human-readable text |
| `review` | `--json` | flag | human-readable text |
| `sync` | `--json` | flag | human-readable text |
| `sync` | `--update[=LIST]` | optional comma-separated `nix`, `rust` list | no upgrades; reconcile Cargo and Bloomery locks |
| all commands | `-h`, `--help` | — | — |

`--json` is a shared global option, accepted before or after the subcommand. It
selects machine-readable output for check, review, and sync; no command-specific
`--format` or alternate JSON spelling is exposed. Human-readable output is the
default. Terminal color behavior is automatic and consistent across commands;
non-empty `NO_COLOR`, redirected streams, and JSON mode suppress ANSI styling.
See the [output contract](output.md).

Bloomery does not expose a command-specific repository-root flag; check and
review use the current working directory, as does sync. The CLI parser also
supplies the `help` subcommand for displaying command help.

For sync, bare `--update` updates Rust dependencies and any existing Nix flake.
An explicit list updates only those ecosystems; explicit `nix` requires
`flake.nix`. Both `--update=nix,rust` and `--update nix,rust` are accepted.
Unknown values, empty elements, an explicit empty list, and repeated flags are
usage errors; duplicate names are deduplicated. Every successful sync still
reconciles Cargo.lock and regenerates bloomery.lock. See the
[sync selection contract](../../SYNC/design/sync.md#invocation-and-update-selection).
