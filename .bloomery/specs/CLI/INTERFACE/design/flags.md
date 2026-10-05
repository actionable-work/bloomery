# Flags and option values

## Shared-purpose options

Shared-purpose flags use the same name and behavior across the application
commands:

| Command | Option | Accepted values | Default |
| --- | --- | --- | --- |
| `check` and nested commands | `--json` | flag | human-readable text |
| `init` | `--json` | flag | human-readable text |
| `init` | `--template` | `basic`, `axum`, or `topcoat` | `basic` workspace template |
| `init` | `--force` | flag | refuse a non-empty target |
| `check`, `check list` | `--check` | repeatable exact ID or quoted glob | all available checks |
| `check`, `check list` | `--system` | repeatable system name | host Nix system |
| `check` | `--jobs` | positive integer | logical CPU count, minimum 1 |
| `check` | `--fail-fast` | flag | finish all selected checks |
| `check failures`, `check details` | `--run` | retained run ID | latest completed workspace run |
| `check list`, `check failures` | `--offset`, `--limit` | nonnegative offset, positive limit | offset 0, limit 20 |
| `check details` | `--offset`, `--limit` | nonnegative offset, positive limit | failure-focused offset, limit 40 |
| `review` | `--json` | flag | human-readable text |
| `sync` | `--json` | flag | human-readable text |
| `config` | `--json` | flag | human-readable text |
| `config list` | `--prefix` | dotted key prefix | all catalogued keys |
| `config upgrade` | `--dry-run` | flag | write the upgraded file |
| `config upgrade` | `--diff` | flag | do not compare recommendations |
| `sync` | `--update[=LIST]` | optional comma-separated `nix`, `rust` list | no upgrades; reconcile Cargo and Bloomery locks |
| all commands | `-h`, `--help` | — | — |

`--json` is a shared global option, accepted before or after the subcommand. It
selects machine-readable output for check, review, sync, and init; no
command-specific `--format` or alternate JSON spelling is exposed. Human-readable
output is the default. Terminal color behavior is automatic and consistent
across commands; non-empty `NO_COLOR`, redirected streams, and JSON mode
suppress ANSI styling.
See the [output contract](output.md).

Bloomery does not expose a command-specific repository-root flag. Check and
review use the current working directory, as does sync. Init instead accepts an
optional positional target directory that defaults to the current working
directory, and its `--template` value names a bundled template. Init refuses a
non-empty target unless `--force` is supplied. Every recognized
command request, including help and nested check retrieval, requires the shared
root `flake.nix` preflight; the workspace-bootstrap `init` command is exempt.
Check, review, and sync additionally require `.bloomery/config.toml`; parser help
and `init` are exempt from the configuration requirement. The CLI parser also
supplies the `help` subcommand for displaying command help.

Check selectors form a union and must each match. `*` and `?` are the supported
case-sensitive anchored glob operators. Offsets are zero-based; limits remain
subject to an 8 KiB serialized-output ceiling. Invalid numbers are usage errors.
Execution-only flags are not accepted on retrieval commands. See
[selection](../../CHECK/design/execution.md) and
[seeking](../../CHECK/design/details.md).

For sync, bare `--update` updates both Rust dependencies and Nix flake inputs.
The shared command preflight always requires `flake.nix`; an explicit list
updates only those ecosystems. Both `--update=nix,rust` and `--update nix,rust`
are accepted.
Unknown values, empty elements, an explicit empty list, and repeated flags are
usage errors; duplicate names are deduplicated. Every successful sync still
reconciles Cargo.lock and regenerates bloomery.lock. See the
[sync selection contract](../../SYNC/design/sync.md#invocation-and-update-selection).

For config, `--prefix` is accepted only by `config list` and restricts the
listing to keys whose dotted path starts with the supplied value; an unknown
prefix yields an empty listing. `--dry-run` is accepted only by `config
upgrade` and reports the keys that would be added without writing, while
`--diff` reports the recommended-versus-current comparison without writing.
`--dry-run` and `--diff` are mutually exclusive and both are usage errors on
other config subcommands. See the
[config editing contract](../../CONFIG/design/editing.md) and
[upgrade contract](../../CONFIG/design/upgrade.md).
