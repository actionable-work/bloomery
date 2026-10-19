# Configuration upgrade

## Schema catalog

`config upgrade` is driven by the
[schema catalog](../../PARSER/CONFIGURATION/design/schema-catalog.md), which
enumerates every configurable option with its type, effective default,
and an optional upgrade recommendation. Recommendations can differ from
effective defaults for check control keys: omission leaves checks disabled or
applies a false check-policy value, while upgrade recommends `true`. The
catalog spans the
CLI-owned `[specs]`/`[scanners.*]` tables and the Nix build tables, and ships
with the Bloomery binary so a newer release exposes keys added since the user's
configuration was written.

Keys whose omission is semantic have no recommendation and are never
materialized: `build.bloomeryLock` auto-detects, `build.members` means all
members, `build.toolchain.linker` is host-dependent,
`features.cratesIoIndex` means none, and the `build.profile.*` fields inherit
from `Cargo.toml` and rustc.

## Format migrations

The top-level integer `version` is required. Format `2` is current. An existing
file without a version is rejected with an error that asks the user to set
`version = 2`; `config upgrade` does not infer or add a version to that file.
Format `0` is unsupported.

`config upgrade` migrates format `1` to format `2` before adding recommended
keys. The migration moves top-level `toolchain`, `profile`, and `flags` tables
under `build`, then moves the flat Nix check controls into their check
subtables: `checks.crateCheck` to `checks.crates.enable`,
`checks.includePackageChecks` to `checks.packages.enable`,
`checks.throwOnOutOfDate` to `checks.lockFreshness.enable`,
`checks.workspaceDependencies` to `checks.workspaceDependencies.enable`, and
`checks.noDefaultFeatures` to `checks.noDefaultFeatures.enable`. The global
`checks.enable` and existing CLI-owned check tables retain their paths. The
migration preserves configured values and their meaning, then writes
`version = 2`. No migration from format `0` or unversioned files is defined.
Adding recommended keys does not itself change the format version.

`--dry-run` reports source and target versions when a supported older numbered
format needs migration, along with catalog keys that would be added, without
writing. `--diff` compares recommendations against the normalized configuration
and reports any applicable format migration without writing. A version `1`
configuration is migrated before it receives newly recommended keys.

## Upgrade semantics

`config upgrade` applies format migrations before defaults are inserted, then
adds every catalog key with an upgrade recommendation that is absent from the
current layout. An absent parent table and an absent leaf inside a present table
both count as missing. Adding a key with its recommended value is written
explicitly. For ordinary keys it matches the effective default; for check
control keys it enables the check or its associated strict policy.

For a file already in the current format, keys already present are preserved
exactly: value, comments, and formatting are not rewritten even when the present
value differs from the recommendation. Format migration may rewrite the layout
of affected keys while preserving their configured values and meaning. In
particular, an explicitly configured `false` check value is never changed. The
command never disables a configured check or deletes configured data. Tables
that are not catalogued pass through unchanged.

`--dry-run` reports the keys that would be added without writing. The report is
sorted by full key path.

## Recommendation diff

`config upgrade --diff` is a read-only comparison between upgrade
recommendations and the current effective configuration. For every catalogued
key that has an upgrade recommendation it reports the recommended value and the
current value, marking each as equal, differing, or absent and would-be-added.
Keys without a recommendation are reported as unset. The diff includes enabled-state
differences such as `checks.enable`, `devShell.enable`, `build.unify`, and
scanner `enabled` flags.

`--diff` writes nothing and is mutually exclusive with `--dry-run`; supplying
both is a usage error. A differing present value is reported but never changed
by `config upgrade`, because explicit configuration always wins.

## Idempotence

Upgrade is idempotent. After a successful upgrade the current format version
and every recommended catalog key are present, so a second invocation makes no
change and leaves the file byte-for-byte unchanged.

## Validation and publication

Upgrade requires a readable configuration with an explicitly supported
version. A malformed, unreadable, invalid, missing, or unsupported version
yields a `ConfigurationError` and no write; a missing version error asks the
user to set `version = 2`. An absent file is not an error: a writing upgrade
creates it at the current format version and materializes every recommended
key, warning while doing so, while `--dry-run` and `--diff` report against
defaults without creating it. The upgraded candidate is revalidated as a
complete configuration before publication and is published through a temporary
file and atomic rename, so a failure leaves the original file intact.

## Output

Human output follows the shared
[presentation grammar](../../INTERFACE/design/presentation.md) and reports the
source and target format versions when migration is pending, plus added key
paths, or states that the configuration is already current. `--diff` prints
the recommendation differences sorted by key path and any pending format
migration. `config upgrade --json` carries the source and target versions when
applicable, added key paths, or recommendation differences as the `result` of
the shared [JSON envelope](../../INTERFACE/design/output.md#json-envelope); a
dry run marks reported changes as not yet written.

Materializing or explicitly setting a key always takes precedence and suppresses
the corresponding [sync recommendation](../../SYNC/design/sync.md#recommendations);
after upgrade, formerly recommended keys are reported as configured rather than
missing.
