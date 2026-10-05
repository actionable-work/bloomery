# Configuration upgrade

## Schema catalog

`config upgrade` is driven by the
[schema catalog](../../PARSER/CONFIGURATION/design/schema-catalog.md), which
enumerates every recognized configuration key with its type and, when omission
has a single fixed effective value, a recommended default. The catalog spans the
CLI-owned `[specs]`/`[scanners.*]` tables and the Nix build tables, and ships
with the Bloomery binary so a newer release exposes keys added since the user's
configuration was written.

Keys whose omission is semantic have no recommendation and are never
materialized: `build.bloomeryLock` auto-detects, `build.members` means all
members, `toolchain.linker` is host-dependent, `features.cratesIoIndex` means
none, and the `profile.*` fields inherit from `Cargo.toml` and rustc.

## Upgrade semantics

`config upgrade` reads the raw file before defaults are inserted and adds every
recommended catalog key that is absent. An absent parent table and an absent
leaf inside a present table both count as missing. Adding a key with its
recommended default does not change the effective configuration, because the
same value would have been applied as a default.

Keys already present are preserved exactly: value, comments, and formatting are
not rewritten even when the present value differs from the recommended default.
The command never enables or disables a feature, deletes a key, or reorders
existing entries. Tables that are not catalogued pass through unchanged.

`--dry-run` reports the keys that would be added without writing. The report is
sorted by full key path.

## Recommendation diff

`config upgrade --diff` is a read-only comparison between the recommended
configuration and the current effective configuration. For every catalogued key
that has a recommended default it reports the recommended value and the current
value, marking each as equal, differing, or absent and would-be-added. Keys
without a recommendation are reported as unset. The diff includes enabled-state
differences such as `checks.enable`, `devShell.enable`, `features.unify`, and
scanner `enabled` flags.

`--diff` writes nothing and is mutually exclusive with `--dry-run`; supplying
both is a usage error. A differing present value is reported but never changed
by `config upgrade`, because explicit configuration always wins.

## Idempotence

Upgrade is idempotent. After a successful upgrade every recommended catalog key
is present, so a second invocation adds nothing and leaves the file
byte-for-byte unchanged. A configuration that already contains every
recommended key is not modified at all.

## Validation and publication

Upgrade requires a valid current configuration. A malformed, unreadable, or
invalid file, including a build-table type or enum error, yields a
`ConfigurationError` and no write. An absent file is not an error: a writing
upgrade creates it and materializes every recommended key, warning while doing
so, while `--dry-run` and `--diff` report against defaults without creating it.
The upgraded candidate is revalidated as a
complete configuration before publication and is published through a temporary
file and atomic rename, so a failure leaves the original file intact.

## Output

Human output reports the added key paths, or states that the configuration is
already current. `--diff` prints the recommendation differences sorted by key
path. `config upgrade --json` emits one document identifying the command,
status, and either the added key paths or the recommendation differences; a dry
run marks the reported paths as not yet written.

Materializing or explicitly setting a key always takes precedence and suppresses
the corresponding [sync recommendation](../../SYNC/design/sync.md#recommendations);
after upgrade, formerly recommended keys are reported as configured rather than
missing.
