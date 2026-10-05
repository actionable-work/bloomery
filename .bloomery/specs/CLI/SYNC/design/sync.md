# Workspace synchronization

## Intent

`bloomery sync` is the single maintenance entry point for `Cargo.lock`,
`bloomery.lock`, and optional `flake.lock` updates. Every sync invocation also
requires the shared CLI `flake.nix` preflight. It replaces the existing Nix lock
generator while leaving Nix's pure lock parsing and validation intact.
The Rust CLI orchestrates tools and owns Bloomery lock generation; running sync
must not depend on evaluating or building the workspace's Nix outputs.

## Invocation and update selection

Run from the workspace root (the current working directory). There is no root
argument or upward workspace search. The root must contain `Cargo.toml` and
`flake.nix`; `Cargo.lock`, `bloomery.lock`, the configuration, and the spec tree
may be absent.

```text
bloomery sync
bloomery sync --update
bloomery sync --update=rust
bloomery sync --update=nix
bloomery sync --update=nix,rust
bloomery sync --update nix,rust
```

`--update` accepts an optional, nonempty comma-separated list of `nix` and
`rust`. Both the equals and separate-argument forms are supported. Unknown
values, empty elements, and an explicitly empty list are usage errors before
mutation. Selection is order-independent; duplicate names are deduplicated.
Repeated `--update` flags are rejected to avoid ambiguous merging.

| Invocation | Rust dependency policy | Nix input policy |
| --- | --- | --- |
| no flag | Reconcile manifests, preserve locked versions where possible | Leave `flake.lock` untouched |
| bare `--update` | Update all dependencies within Cargo manifest constraints | Update all Nix inputs |
| `--update=rust` | Update all dependencies within Cargo manifest constraints | Leave `flake.lock` untouched |
| `--update=nix` | Reconcile manifests, preserve locked versions where possible | Update all Nix inputs |
| `--update=nix,rust` | Update all dependencies within Cargo manifest constraints | Update all Nix inputs |

Bare `--update` selects both Rust and Nix updates. The shared CLI preflight
rejects every command when the root lacks `flake.nix`. Every successful
invocation reconciles `Cargo.lock` and regenerates `bloomery.lock`, even when
only Nix was selected. `bloomery.lock` is a derived resolution artifact, not a
separately selectable update ecosystem. No package-specific or input-specific update
selection is included in this design.

## Execution pipeline

1. Parse CLI arguments and determine the root.
2. Preflight: require `Cargo.toml` and `flake.nix`, load/validate configuration
   if present, and verify required executables. Cargo is always required; Nix is
   required only when an actual Nix update will run. A present malformed,
   unreadable, or invalid config remains an error under the shared configuration
   contract. Preflight failures occur before invoking mutating tools.
3. Emit the missing-config warning or the recommendation notices described
   below. These do not gate synchronization.
4. Perform selected Nix updates first via `nix flake update` in the root. If
   needed, Nix creates `flake.lock`. Normal sync does not invoke Nix at all.
5. For a Rust update, run `cargo update --manifest-path <root>/Cargo.toml`.
   Otherwise resolve via `cargo metadata --manifest-path <root>/Cargo.toml
   --format-version 1`, without `--locked`, allowing Cargo to repair or create
   `Cargo.lock`. This reconciles manifest changes without deliberately upgrading
   already compatible locked dependencies. Do not delete the lock or run
   `cargo generate-lockfile` as the normal refresh strategy.
6. Obtain final metadata via `cargo metadata --locked --manifest-path
   <root>/Cargo.toml --format-version 1`. This must not further mutate the Cargo
   resolution. Generate the Bloomery lock from this snapshot and the final
   `Cargo.lock` hash.
7. Publish the generated lock safely and report completion.

Cargo's normal workspace resolution and feature behavior are retained; sync
adds no target or feature selection flags. Normal reconciliation may change
versions when manifest changes require it and may access the network. It is
not an offline mode and does not promise that Cargo.lock will be unchanged.
Neither sync nor update modifies dependency constraints in Cargo.toml.

Sync dispatch must happen before specification loading in the CLI. Missing or
invalid specs and uncovered requirements cannot prevent lock repair. This is
separate from rejecting an invalid configuration file.

## Bloomery lock format and publication

Preserve the existing version-1 TOML contract consumed by the Nix builders:

- Generated-file header and `version = 1`.
- `cargo-lock-hash`: SHA-256 of final `Cargo.lock`, normalizing CRLF to LF,
  matching the Nix lock validator.
- A `packages` table keyed by `name-version`, covering resolved packages with
  feature names, dependency package identifiers, `proc-macro`, and edition.
- Derive package records from Cargo metadata's resolved graph. Retain the
  current lock generator's field semantics and edition fallback (`2021`).

Canonical output sorts package keys, feature names, and deduplicated dependency
identifiers and correctly escapes TOML strings. Identical metadata and Cargo
lock bytes produce identical Bloomery lock bytes. If multiple distinct Cargo
package identities collide on a `name-version` key, fail with an actionable
unsupported-resolution error instead of silently overwriting a record. A future
lock format change to represent that case is out of scope.

Serialize and validate the entire candidate before writing. Write to a temporary
file beside `bloomery.lock`, then atomically rename into place; clean up temporary
files on failure. Leave an existing identical lock unchanged. Failures before
publication preserve the previous Bloomery lock (or leave it absent if none
existed).

## Recommendations

Use a built-in, versioned-with-the-binary catalog. Each entry contains an exact
TOML key path, a short benefit description, and actionable configuration
guidance. Entries must refer to keys supported by that Bloomery release;
recommendations do not imply a new config schema or remotely fetched policy.
This advisory subset is derived from the CLI-owned recommended-default catalog
defined by
[PARSER/CONFIGURATION](../../PARSER/CONFIGURATION/design/config-file.md#recommended-default-catalog);
the corresponding default value is materialized by `bloomery config upgrade`.

The initial catalog targets existing check/evidence scanner switches:

| Key | Benefit / guidance |
| --- | --- |
| `scanners.rust.enabled` | Rust evidence scanning; configure repository-relative `paths` when enabling |
| `scanners.playwright.enabled` | Playwright evidence scanning; configure repository-relative `paths` when enabling |
| `scanners.nix.enabled` | Nix check evidence scanning; configure repository-relative `paths` for `passthru.bloomery` metadata when enabling |

These are the existing scanner keys associated with check evidence. The Nix
build `[checks]` table is a separate build configuration surface and is not part
of this recommendation catalog. Recommendations suggest evaluating
applicability, not blindly enabling every scanner. No repository-language
heuristics filter this initial catalog.

Parse the config into a raw TOML value tree as well as the validated typed
configuration. Compare each catalog path to the raw tree **before default
insertion**. An absent leaf is a recommendation even if its parent table exists;
an absent parent means its descendants are absent. An explicit leaf suppresses
that recommendation, including `false`, `true`, or any other schema-valid
value. Values materialized by `bloomery config upgrade` are explicit and
therefore suppress notices; materialized or user-authored config always wins. An
invalid value is a config error, not an absence. TOML dotted keys and
inline tables have the same presence semantics as regular tables.

For example:

```toml
[scanners.rust]
enabled = false

[scanners.playwright]
# enabled intentionally not configured yet
```

This suppresses the Rust notice and reports the Playwright and Nix keys. A
present empty config reports all three. Missing auxiliary keys such as `paths`
are not independent recommendations unless they become catalog entries later.

Sort notices by full key path and emit each at most once per invocation. Describe
an absent entry as “recommended feature not yet configured,” not necessarily a
feature introduced since the user's last run. Repeat on later syncs until the
key is configured, including explicit disablement. Store no seen/version state;
new catalog entries naturally appear for absent keys after a CLI upgrade.

## Missing configuration

`.bloomery/config.toml` is required. When it is absent, sync stops before
reconciliation and reports a `ConfigurationError` naming the path and
recommending creation of the file. Sync does not create or edit `.bloomery/` or
the config automatically. A dangling config symlink or another read failure is
also an error, not a missing-file default. Missing specs do not change this
behavior.

Example error:

```text
error: .bloomery/config.toml is required; create it to configure Bloomery before running sync.
```

## Failures and output

Return `0` only after all requested operations and Bloomery lock publication
succeed. Warnings and recommendations alone do not change success. Return `1`
for configuration, prerequisite, tool, generation, or write failures, and `2`
for CLI usage errors. Identify the failing stage, retain useful subprocess
stderr, and do not report completion or run later stages after a failure.

Use stdout for stage progress and completion, stderr for warnings,
recommendations, and failures. A successful summary identifies reconciled
locks and selected update ecosystems. `sync --json` emits the structured success
or failure contract
specified by [CLI output design](../../INTERFACE/design/output.md). No
interactive confirmation is introduced. Invoke subprocesses with structured
arguments rather than interpolated shell commands.

There is no cross-lock transaction: Cargo and Nix own their writes. If a later
stage fails, earlier lockfile updates may remain. Report completed stages and
warn that locks may be partially synchronized; advise rerunning `bloomery sync`
after fixing the error. Never roll back tool-owned locks or overwrite the old
Bloomery lock with partial data. Concurrent runs in the same root are unsupported
in this initial design; callers must serialize them.

## Nix migration

Remove `apps.lock` and the old lock-app/script exports throughout Bloomery's
flake and workspace construction API, including `mkFlake`. Do not introduce
`apps.sync`, a `lock` alias, or a `bloomery lock` subcommand. This is a deliberate
breaking removal; unrelated app outputs are unaffected.

Documentation and lock-validator repair messages use `bloomery sync`; consumers
obtain the command from Bloomery's `bloomery` package. Nix lock validation
remains read-only and does not invoke sync automatically during builds or
evaluation.

## Verification plan

Use temporary workspace fixtures and injectable subprocess runners for the
shared flake preflight, update selection, tool failures, and partial-success
reporting. Exercise real Cargo fixtures for missing and stale locks, version
preservation, explicit updates, and compatible metadata generation. Test
byte-level lock stability, escaping, collision rejection, CRLF hash agreement
with the Nix consumer, and safe publication under injected write failures.

Use config fixtures for absent files, empty files, missing parents/leaves,
explicit false/true, dotted keys, inline tables, malformed files, and future
catalog entries. Assert stable notices and no config mutation. Verify sync works
with missing or uncovered specs when a root flake is present, and rejects a
missing root flake before invoking tools or mutating locks. Nix integration tests
assert lock-app exports are absent across consumption styles while normal
workspace apps and lock validation remain available.
