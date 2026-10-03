# Grouped requirement format

Each requirement group is a TOML file at:

```text
.bloomery/specs/<AREA>/<FEATURE>/requirements/<GROUP>.toml
```

The root `group` string must equal the file stem. The file contains one or more
`[[requirements]]` entries. A record contains an `id`, human-readable `title`,
boolean `manual`, optional `design` reference, and a typed nested `ears` table.

The `design` path is relative to the feature directory. If omitted, it resolves
to the feature README. An anchor may be appended after `#`.

## EARS clauses

The `ears.type` value selects the required fields and canonical sentence:

| Type | Required fields | Canonical form |
| --- | --- | --- |
| `ubiquitous` | `system`, `action` | The `<system>` shall `<action>`. |
| `event` | `trigger`, `system`, `action` | When `<trigger>`, the `<system>` shall `<action>`. |
| `state` | `state`, `system`, `action` | While `<state>`, the `<system>` shall `<action>`. |
| `unwanted_behavior` | `trigger`, `system`, `action` | If `<trigger>`, then the `<system>` shall `<action>`. |
| `optional` | `feature`, `system`, `action` | Where `<feature>`, the `<system>` shall `<action>`. |
| `complex` | `state`, `trigger`, `system`, `action` | While `<state>`, when `<trigger>`, the `<system>` shall `<action>`. |

Unknown types and missing fields are grammar errors. The canonical statement is
derived from structured fields for review output; it is not stored as a second,
potentially divergent string.

## Verification modes

`manual = false` means the requirement must have at least one statically
discovered test reference. `manual = true` means it is intentionally excluded
from automated coverage and appears in `bloomery review`. A single record may
not serve both modes.
