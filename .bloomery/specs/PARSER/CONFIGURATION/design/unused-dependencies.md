# Unused dependency policy configuration

`[checks.unusedDependencies]` is CLI-owned configuration consumed by
[`static:unused-dependencies`](../../../CLI/CHECK/design/unused-dependencies.md).
The Nix loader accepts and validates the table without generating an unused
dependency derivation.

| Key | Type | Effective default | Upgrade recommendation |
| --- | --- | --- | --- |
| `enable` | boolean | `false` | `true` |
| `exceptions` | list of Cargo package names | `[]` | `[]` |

Each `exceptions` entry is a bare Cargo package name without a version.
Duplicate entries are rejected. Unknown keys, malformed names, and invalid
types fail as `ConfigurationError` before formatting or analysis. An omitted
`enable` key leaves the check off, including in existing configuration files.
Configuration upgrade recommends `true` for the missing enable key and
preserves any configured value. Validation applies even when the check is
disabled.
