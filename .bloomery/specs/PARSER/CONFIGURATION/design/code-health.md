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
| `maxRegression` | number in 0–100 | `2` | `2` |
| `cyclomaticLimit` | positive integer | `10` | `10` |
| `functionTokensLimit` | positive integer | `250` | `250` |
| `nestingLimit` | positive integer | `4` | `4` |
| `crateSlocLimit` | positive integer production SLOC | `1500` | `1500` |
| `duplicationFreeRatio` | number in [0,1) | `0.02` | `0.02` |
| `duplicateMinTokens` | integer at least 16 | `64` | `64` |
| `maxCyclomatic` | positive integer | `40` | `40` |
| `maxFunctionTokens` | positive integer | `1000` | `1000` |
| `maxNesting` | positive integer | `8` | `8` |
| `maxCrateSloc` | positive integer production SLOC | `5000` | `5000` |
| `crateExceptions` | table keyed by workspace-relative `Cargo.toml` paths | empty | empty |

The nested `[checks.codeHealth.weights]` table carries relative score weights:

| Key under `checks.codeHealth.weights` | Type | Effective default | Upgrade recommendation |
| --- | --- | --- | --- |
| `complexity` | finite number in 0–100, at most two decimals | `25` | `25` |
| `size` | finite number in 0–100, at most two decimals | `20` | `20` |
| `nesting` | finite number in 0–100, at most two decimals | `10` | `10` |
| `duplication` | finite number in 0–100, at most two decimals | `15` | `15` |
| `crateSize` | finite number in 0–100, at most two decimals | `30` | `30` |

Omitted weight keys use their defaults. The effective weights are normalized by
their sum when scoring, and at least one effective weight must be greater than
zero. Zero disables that component. The same effective weights apply to each
selected crate; workspace scores remain SLOC-weighted means of crate scores.
The `crateSize` component scores production SLOC above `crateSlocLimit`; any
crate above `maxCrateSloc` fails even when its score passes. This makes crate
size affect crate and workspace scores while preserving SLOC-weighted workspace
aggregation. Optional exception tables may override any subset of
`maxCyclomatic`, `maxFunctionTokens`, `maxNesting`, and `maxCrateSloc` for one
exact workspace-relative Cargo manifest path. Omitted fields inherit their
global hard limits. Exception paths must name `Cargo.toml`; each override must
be a positive integer at least as large as its corresponding global soft
limit. Unknown exception fields and malformed paths are rejected. Both CLI and
flake validation accept the exception table; CLI loading resolves the
effective health policy once.

Scores, regression allowances, and floors permit at most two decimal places and
use hundredths of a point. Ratios are finite TOML numbers. Integer fields
reject floats. Hard limits must be at least their corresponding soft limits,
including `maxCrateSloc >= crateSlocLimit`. Reject unknown keys, invalid types,
nonfinite numbers, invalid ranges, all-zero weights, and invalid baseline paths
as `ConfigurationError` before formatting or analysis. Validation applies even
when health is disabled.

The 2-point regression default allows ordinary score movement in small crates
while absolute score floors and hard limits remain in force.

The baseline is a portable measurement artifact, separate from the immutable
configuration and specification tree. Its parent may be created by the explicit
update command. Relative paths must remain within the repository, cannot
overlap analysis inputs or managed config/lock/spec files, and must not escape
through symlinks. The default filename makes the reviewed artifact suitable
for version control. Complete write-mode health checks refresh it; no-write
checks read it without replacement, and retrieval commands never modify it.

Enablement is independent of `specs.enable`, evidence-scanner switches, and
`checks.enable`. An omitted table applies the effective defaults. The check is
off when its `enable` key is omitted; configuration upgrade recommends `true`
to explicitly opt into it and preserves any configured value.
