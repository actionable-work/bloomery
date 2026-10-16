# Clippy suppression policy configuration

`[checks.clippySuppressions]` is CLI-owned configuration consumed by
[`static:clippy-suppressions`](../../../CLI/CHECK/design/clippy-suppressions.md).
The Nix loader accepts and validates the table without generating a Clippy
suppression derivation.

| Key | Type | Effective default | Upgrade recommendation |
| --- | --- | --- | --- |
| `enable` | boolean | `false` | `true` |
| `allow` | list of qualified Clippy lint names | `[]` | `[]` |

Each `allow` entry uses the exact `clippy::lint_name` spelling from a Rust lint
attribute. Duplicate entries are rejected. Unknown keys, malformed names, and
invalid types fail as `ConfigurationError` before formatting or analysis.
An omitted `enable` key leaves the check off, including in existing
configuration files. Configuration upgrade recommends `true` for the missing
enable key and preserves any configured value. Validation applies even when the
check is disabled.
