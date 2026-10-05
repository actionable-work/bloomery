# Typesafe configuration editing

## Invocation

`bloomery config` is exempt from both the root `flake.nix` preflight and the
required-configuration preflight. It edits or creates `.bloomery/config.toml`
even when the flake is absent, emitting an advisory warning naming the missing
Bloomery flake. The command tree is:

```text
bloomery config get KEY [--json]
bloomery config set KEY VALUE [--json]
bloomery config unset KEY [--json]
bloomery config list [--prefix KEY] [--json]
bloomery config upgrade [--dry-run | --diff] [--json]
bloomery config document [--json]
```

`config document` annotates configured keys with schema-catalog documentation
and is specified by [Configuration documentation](documenting.md).

Every invocation re-reads the configuration file. Config commands never load
the specification tree, scan source, or edit lockfiles.

## Key addressing and type safety

`KEY` is a dotted TOML path whose segments are bare TOML keys, for example
`checks.enable` or `scanners.rust.paths`. The path resolves against the
[schema catalog](../../PARSER/CONFIGURATION/design/schema-catalog.md), which
assigns every recognized key a type and, when omission has a single fixed
effective value, a recommended default. An unknown path, including an unknown
key in a build table, is a usage error before any read or write.

`VALUE` is parsed according to the addressed key's schema type:

| Schema type | Accepted input |
| --- | --- |
| boolean | `true` or `false` |
| integer | a decimal integer |
| string | a literal argument value |
| list of strings | a TOML array literal such as `["*.rs", "src/**/*.rs"]` |
| enum or union | one of the catalogued literals, such as `"fat"` or `true` for `profile.release.lto` |

A value that does not parse as the schema type, an enum value outside the
catalogued set, or an empty array where the schema forbids one is a usage error
before any write.

## Missing configuration

When `.bloomery/config.toml` is absent, the mutating commands `config set`,
`config unset`, a writing `config upgrade`, and `config document` create the
`.bloomery/` directory and an empty configuration file, emit an advisory
warning naming the created path, and continue. The effective configuration
before the mutation is the documented defaults, and the newly created file is
an empty TOML document; the command's own keys are then added in the same
invocation. `config document` has no present keys to annotate in the new file.

The read-only commands `config get`, `config list`, `config upgrade --dry-run`,
and `config upgrade --diff` never create or modify the file. They report
against defaults and emit the same advisory warning.

## Read commands

`config get` addresses one key and reports its effective value together with
whether the key is explicitly present in the raw file. An absent key reports
its documented default and is marked as not explicitly configured; a key with
no recommended default reports that it is unset.

`config list` enumerates every catalogued key with its effective value,
configured status, and whether a recommended default exists. `--prefix`
restricts the listing to keys whose dotted path starts with the supplied
prefix; an unknown prefix yields an empty listing rather than an error.

## Mutation semantics

`config set` is an upsert. It replaces a present leaf and creates any missing
parent tables. `config unset` removes a present leaf. Removing the final leaf
of a parent table removes that now-empty table so the file does not accumulate
empty headers.

Mutations edit only the addressed subtree. Present keys, their comments, and
the surrounding formatting are preserved; a key that is already equal to the
requested value is left unchanged. `config unset` on an absent key and
`config set` of an unchanged value succeed without a diff.

## Atomicity and validation

A mutation builds the candidate document in memory, then deserializes it into
the typed configuration to confirm the complete result is valid, including the
build-table type and enum rules. Invalid candidates are a `ConfigurationError`
and no write occurs. The same validation runs when a command loads an existing
file, so a mistyped build key never reaches Nix evaluation.

The candidate is written to a temporary file beside `.bloomery/config.toml`
and atomically renamed into place. Any failure before the rename leaves the
existing file unchanged. Config commands do not retain partial edits or repair
a malformed file; a malformed or unreadable file is a `ConfigurationError`.

## Output and exit codes

Human output names the command, the affected key path, and the effective or
changed value. Missing-flake and missing-configuration warnings are advisory
and do not change the exit code. `config list` and `config upgrade` print
sorted key paths.

`config --json` emits one structured document. Success documents identify the
command, status, and command-specific fields: a key with an effective value,
configured flag, and recommendation status for `get`, a key array for `list`,
the changed key and whether the file changed for `set` and `unset`, and the
added key paths or recommendation differences for `upgrade`.

Usage errors return exit code 2 before any mutation. Configuration read,
validation, or write failures return exit code 1. Successful reads and
mutations return 0. Missing-flake and missing-configuration warnings alone
still return the command's normal success code.
