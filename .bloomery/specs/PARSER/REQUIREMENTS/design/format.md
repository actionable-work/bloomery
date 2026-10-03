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

## Requirement granularity

Each requirement specifies one independently verifiable obligation. Split
command exposure, argument syntax, defaults, mutation policy, output, and
failure handling into separate records rather than combining them in one
`action`. Sharing a trigger or design reference does not make independent
obligations one requirement.

For example, exposing `check`, exposing `review`, exposing `sync`, providing
help, and selecting the repository root are five requirements. Returning a
failure code, stopping later stages, and reporting recovery guidance are also
separate requirements.

Keep closely related inputs together when they exercise the same obligation:
several invalid-config conditions may all require the same `ConfigurationError`,
and sorting plus deduplication may define one canonical collection. Do not
split merely at every conjunction; split where a behavior can pass or fail
independently. Test cases may cover several requirements, but each requirement
must remain independently traceable.

When decomposing an existing record, retain its ID for one narrowed obligation
and allocate new IDs for the others without renumbering unrelated records.
Preserve verification modes; decomposition does not justify placeholder test
evidence or changing an automated requirement to manual.

## Verification modes

`manual = false` means the requirement must have at least one statically
discovered test reference. `manual = true` means it is intentionally excluded
from automated coverage and appears in `bloomery review`. A single record may
not serve both modes.
