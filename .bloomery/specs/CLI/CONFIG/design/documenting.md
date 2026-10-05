# Configuration documentation

`bloomery config document` annotates `.bloomery/config.toml` with the
schema-catalog documentation text for every key the file already configures. It
changes comments only: values, present keys, and unrelated formatting stay as
they are.

## Documentation source

Documentation text comes from the
[schema catalog](../../PARSER/CONFIGURATION/design/schema-catalog.md#catalog-fields).
Every catalog entry carries a documentation string, and the command renders that
string as a TOML comment rather than maintaining a second copy of the prose. A
key outside the catalog is never documented.

## Inline comments

The catalog documentation is written as a trailing comment on the key's value:

```toml
cargoToml = "Cargo.toml" # repository-relative path to the Cargo manifest
```

A trailing comment renders after the complete value, so a multi-line array
receives its documentation after the closing bracket. When the value already
carries a trailing comment, the existing comment is preserved and the catalog
documentation is written on the line above the key instead. That fallback keeps
the user's trailing note while still documenting the key.

## Annotation semantics

`config document` writes one catalog documentation comment for each key
explicitly present in the raw file. It does not add, remove, or rewrite keys:
absent catalog keys stay absent, and `config upgrade` remains the command that
materializes recommended defaults. A present value is never changed, including a
value that differs from its recommended default. A table header is not itself
documented; only leaves receive a documentation comment.

Existing comments are preserved. The command does not remove or rewrite
unrelated comment lines.

## Idempotence

Documentation is idempotent. A key is already documented when the catalog
documentation appears as its trailing comment or on the line above it; such a
key is left untouched. A repeated invocation therefore adds nothing and leaves
the file byte-for-byte unchanged.

## Missing configuration

`config document` is a mutating command. When `.bloomery/config.toml` is absent
it creates the `.bloomery/` directory and an empty configuration file, emits the
advisory missing-configuration warning, and reports that no keys were
documented.

## Validation and publication

The command edits only the comment layer and then deserializes the complete
result into the typed configuration to confirm it is still valid. An invalid
result is a `ConfigurationError` and no write occurs. The annotated document is
published through a temporary file and atomic rename, so a failure leaves the
existing file unchanged.

## Output and exit codes

Human output names the command, the count of documented keys, and the sorted key
paths that gained documentation. `config document --json` emits one document
identifying the command, status, the sorted documented key paths, and whether
the file changed. Documented keys are reported in sorted order.

Usage errors return exit code 2 before any mutation. Configuration read,
validation, or write failures return exit code 1. A successful run returns 0,
including when the file already carried every documentation comment.
