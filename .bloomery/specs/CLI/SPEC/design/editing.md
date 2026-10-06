# Record editing

`bloomery spec` creates, reads, updates, and removes individual requirement
records. It requires the shared root `flake.nix` and `.bloomery/config.toml`
preflights, then loads the specification tree at
`.bloomery/<specs.dir>/`.

## Invocation

```text
bloomery spec list [--area AREA] [--feature FEATURE] [--group GROUP] [--json]
bloomery spec show ID [--json]
bloomery spec add ID --title TITLE --ears TOML [--design PATH] [--manual] [--json]
bloomery spec set ID FIELD VALUE [--json]
bloomery spec remove ID [--json]
```

Traceability commands are specified in [Traceability projections](trace.md).

## Identifier-derived location

A requirement ID encodes its physical location:

```text
<AREA>-<FEATURE>-<GROUP>-<SEQUENCE>
  │      │       │        └── sequence within the group
  │      │       └────────── .bloomery/<specs.dir>/<AREA>/<FEATURE>/requirements/<GROUP>.toml
  │      └────────────────── feature README and design directory
  └───────────────────────── area README
```

`spec add` requires the area and feature READMEs to already exist and never
creates them. It creates the group file when absent and inserts the record in
sequence order. `spec set` and `spec remove` resolve the record through the
loaded tree rather than by re-parsing the ID.

## Reads

`spec list` enumerates records with their area, feature, group, ID, title,
verification mode, and design reference. `--area`, `--feature`, and `--group`
restrict the listing to matching identifiers; an unmatched filter yields an
empty listing rather than an error.

`spec show` reports one record's fields, its canonical EARS statement, and its
resolved design reference. An unknown
ID is a usage error before any other work.

## Additions

`spec add` accepts a full record. `--title` supplies the title. `--ears`
supplies the EARS table as a TOML inline table, for example
`{ type = "event", trigger = "x", system = "y", action = "z" }`. `--manual`
marks the record as human-reviewed and defaults to automated verification.
`--design` supplies a feature-relative path with an optional `#anchor`; when
omitted, the record resolves to the feature README.

An ID that already exists, a malformed ID, an unknown EARS type, a missing EARS
field, a design path that does not resolve, or a sequence collision is a usage
error and nothing is written.

## Field updates

`spec set` replaces one addressed field. `FIELD` is one of:

| Field | Value |
| --- | --- |
| `title` | literal title |
| `manual` | `true` or `false` |
| `design` | feature-relative path with optional anchor, or an empty value to clear |
| `ears` | TOML inline table replacing the complete EARS clause |

The candidate record is validated under the full grammar, including the EARS
type's required fields. Changing `ears` replaces the clause atomically so no
stale field survives a type change.

## Removal

`spec remove` deletes the addressed record. When the group file becomes empty it
is removed so no empty group remains. Removal refuses to delete the last
requirement group of a feature, because every feature must retain at least one
grouped requirements file; the error names the feature. Design documents and
area or feature READMEs are never touched.

## Preservation and atomicity

Every mutation reads the grouped file, edits only the addressed record, and
preserves unrelated records, comments, key order, and formatting. The complete
resulting specification tree is validated, then the changed group file is
written through a temporary file and atomic rename. A failure before the rename
leaves the existing tree unchanged. Spec mutations never load evidence or scan
source, and never edit configuration or lockfiles.

## Output and exit codes

Human output names the command, the affected requirement ID, and the resulting
fields or change. `spec list` prints records in deterministic order by area,
feature, group, and numeric sequence, matching the review catalog.

Usage errors, including an unknown ID, an unknown field, an unparsable value, a
duplicate ID, and an invalid resulting record, return exit code 2 before any
write. Read, validation, or write failures return exit code 1. Successful reads
and mutations return 0.