# Unused dependency policy configuration

`[checks.unusedDependencies]` configures both unused dependency checks.
`enable` selects the CLI-owned
[`static:unused-dependencies`](../../../CLI/CHECK/design/unused-dependencies.md)
check. `compiler` selects the compiler-backed
[`<crate>:unused-dependencies`](../../../NIXLIB/GRAPH/design/unused-dependency-checks.md)
Nix checks, which the Nix loader maps to `checks.unusedDependencies.compiler`
and `checks.unusedDependencies.exceptions`. `exceptions` applies to both.

| Key | Type | Effective default | Upgrade recommendation |
| --- | --- | --- | --- |
| `compiler` | boolean | `false` | `true` |
| `enable` | boolean | `false` | `true` |
| `exceptions` | list of Cargo package names | `[]` | `[]` |

Each `exceptions` entry is a bare Cargo package name without a version.
Duplicate entries are rejected. Unknown keys, malformed names, and invalid
types fail as `ConfigurationError` before formatting or analysis. An omitted
`enable` or `compiler` key leaves that check off, including in existing
configuration files. Configuration upgrade recommends `true` for each missing
switch and preserves any configured value. Validation applies even when the check is
disabled.
