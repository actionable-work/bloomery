# Code health configuration

`[checks.codeHealth]` is CLI-owned configuration consumed by
[`static:code-health`](../../../CLI/CHECK/design/code-health.md). The parent
`[checks]` retains its Nix build keys alongside CLI-owned policy subtables. The
flake loader accepts and validates the nested table but does not generate a
health derivation or interpret its enablement as a Nix check toggle.

## Catalog

Every key is addressable through `bloomery config` and documented by the shared
schema catalog, with these effective defaults and upgrade recommendations:

| Key under `checks.codeHealth` | Type | Effective default | Upgrade recommendation |
| --- | --- | --- | --- |
| `enable` | boolean | `false` | `true` |
| `baseline` | repository-relative file path | `".bloomery/code-health-summary.json"` | `".bloomery/code-health-summary.json"` |
| `minCrateScore` | number in 0–100 | `80` | `80` |
| `minWorkspaceScore` | number in 0–100 | `80` | `80` |
| `maxRegression` | number in 0–100 | `0` | `0` |
| `cyclomaticLimit` | positive integer | `10` | `10` |
| `functionTokensLimit` | positive integer | `250` | `250` |
| `nestingLimit` | positive integer | `4` | `4` |
| `duplicationFreeRatio` | number in [0,1) | `0.02` | `0.02` |
| `duplicateMinTokens` | integer at least 16 | `64` | `64` |
| `maxCyclomatic` | positive integer | `40` | `40` |
| `maxFunctionTokens` | positive integer | `1000` | `1000` |
| `maxNesting` | positive integer | `8` | `8` |

Thresholds apply uniformly to selected crates. No per-crate exceptions,
suppression attributes, or customizable weights are part of the policy.
Scores, regression allowances, and floors permit at most two decimal places
and use hundredths of a point. Ratios are finite TOML numbers. Integer fields
reject floats. Hard limits must be at least their corresponding soft limits.
Reject unknown keys, invalid types, nonfinite numbers, invalid ranges, and
invalid baseline paths as `ConfigurationError` before formatting or analysis.
Validation applies even when health is disabled. Both CLI and flake validation
accept this table; CLI loading resolves the effective health policy once.

The baseline is a portable measurement artifact, separate from the immutable
configuration and specification tree. Its parent may be created by the explicit
update command. Relative paths must remain within the repository, cannot
overlap analysis inputs or managed config/lock/spec files, and must not escape
through symlinks. The default filename makes the reviewed artifact suitable
for version control. Ordinary checks read it without writing it.

Enablement is independent of `specs.enable`, evidence-scanner switches, and
`checks.enable`. An omitted table applies the effective defaults. The check is
off when its `enable` key is omitted; configuration upgrade recommends `true`
to explicitly opt into it and preserves any configured value.
