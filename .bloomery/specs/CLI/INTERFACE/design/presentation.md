# Human output presentation

All application commands render human output through one presentation grammar.
The grammar is defined once here; command-specific designs describe only their
content. Human output is a readability layer, not a machine contract; `--json`
is the machine interface.

## Principles

- Terse: one idea per line, no repeated banners, no decorative filler.
- Scannable: stable labels, aligned values, one status vocabulary, whitespace
  between sections.
- Consistent: every command uses the same primitives for headings, trees,
  statuses, notices, and summaries.
- Pipe-safe: styling never changes text, ordering, or values; stripping ANSI
  yields the same plain output.

## Visual grammar

### Headings

A command opens with one root heading naming its subject, never the executable
and never a repeated per-line banner. Nested structure indents under the root
heading with the tree grammar instead of additional heading lines. Root headings
use the `heading` role.

### Trees

Hierarchical output uses one tree grammar:

- the root label occupies its own line with no connector;
- each nested node is prefixed with `└── ` and indented four spaces per level;
- a node may be followed by detail lines indented beneath it without a
  connector;
- a label and its value are separated by `: `.

The review catalog and `disk tree` are the canonical instances:

```text
PARSER
└── REQUIREMENTS
    └── AUTH
        └── [PARSER-REQUIREMENTS-AUTH-001] Human review title
            While the account is locked, the login UI shall ...
            Design: .bloomery/specs/PARSER/REQUIREMENTS/README.md
```

### Status lines

Flat outcomes use one vocabulary. Each outcome line carries a status glyph and
an always-present text label, so plain output remains readable and parseable:

| Glyph | Label | Meaning |
| --- | --- | --- |
| `✓` | `passed` | successful outcome |
| `✗` | `failed` | failed outcome |
| `!` | `warning` | advisory condition |
| `ℹ` | `note` | informational condition |
| `·` | `skipped` | neutral or not-run outcome |

Multiple outcomes on one line are separated by ` · `, for example
`✓ passed · 28 passed · 2 failed`. Glyphs are presentation only; the text label
is authoritative and always present.

### Color roles

Styling is applied by semantic role, never by ad hoc per-command prefixes.
Every command draws from one palette:

| Role | SGR | Applied to |
| --- | --- | --- |
| `heading` | `1` | root headings and titles |
| `success` | `1;32` | passed outcomes |
| `failure` | `1;31` | failed outcomes and failure identifiers |
| `warning` | `1;33` | warnings and advisories |
| `info` | `36` | source locations, paths, and notes |
| `accent` | `1;36` | stage markers, tree connectors, and progress |
| `identifier` | `1;35` | requirement, check, and failure IDs |
| `muted` | `2` | de-emphasized metadata |

### Key-value and summary lines

- A label and its value use `Label: value` with the label styled by role and the
  value unstyled.
- Every command closes its human output with one summary line carrying the
  command's primary result and counts.
- Summaries are not colored wholesale; their status glyph and label use the
  status vocabulary.

### Notices

- Primary results, headings, trees, statuses, and summaries are written to
  stdout.
- Errors and warnings are written to stderr with stable `error:` and
  `warning:` prefixes; informational notices use `note:`.
- Warnings never interrupt or replace the primary result.

### Spacing

- One blank line separates the root heading from the first section and
  independent sections from each other.
- Output ends with the summary line and no trailing blank line.
- Structure is not repeated per record unless the record is the unit of the
  command.

### Progressive disclosure

- Default output is bounded and omits unbounded detail.
- Omit zero-valued, empty, default, or repeated metadata when it adds no
  information or follow-up action.
- Show page range metadata once and show a continuation offset only when another
  page exists.
- Full diagnostics, tool output, and per-record success detail are retrieved
  through dedicated commands or `--json`, not printed by default.
- Mutating commands report the affected identity and resulting state before the
  summary.

## Non-interactive and color control

- ANSI styling, cursor control, and transient progress are never written to a
  stream unless that stream is eligible for them.
- A stream is eligible for styling when it is a terminal, `NO_COLOR` is absent
  or empty, `TERM` is not `dumb`, and `--json` is not selected.
- `--color=auto` (the default) honors the eligibility rules above.
  `--color=always` forces styling of human output even on a redirected stream;
  `--json` still emits no ANSI. `--color=never` disables styling.
- Plain output preserves every label, connector, ordering, and summary value;
  only escape sequences and transient redraws are removed.
