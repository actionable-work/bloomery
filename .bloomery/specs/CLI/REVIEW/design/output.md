# Review output

## Human-readable output

The default output presents the hierarchy as a tree and includes the title,
canonical EARS statement, and design path for each manual requirement. It is the
canonical instance of the shared
[tree grammar](../../INTERFACE/design/presentation.md#trees): a root area label,
`└──`-prefixed nested nodes indented four spaces per level, and detail lines
indented beneath the requirement node. Area and feature headings, requirement
IDs, and the final count use the shared
[presentation roles](../../INTERFACE/design/presentation.md#color-roles); non-empty
`NO_COLOR`, redirected output, `--color=never`, and JSON mode disable ANSI
styling.

```text
PARSER
└── REQUIREMENTS
    └── AUTH
        └── [PARSER-REQUIREMENTS-AUTH-001] Human review title
            While the account is locked, the login UI shall ...
            Design: .bloomery/specs/PARSER/REQUIREMENTS/README.md

Total manual requirements requiring review: 1
```

## JSON output

`bloomery review --json` carries the catalog as the `result` of the shared
[JSON envelope](../../INTERFACE/design/output.md#json-envelope) with stable field
names and no ANSI escape sequences:

```json
{
  "command": "review",
  "status": "succeeded",
  "result": [
    {
      "area": "PARSER",
      "feature": "REQUIREMENTS",
      "group": "AUTH",
      "id": "PARSER-REQUIREMENTS-AUTH-001",
      "title": "Human review title",
      "statement": "While the account is locked, the login UI shall ...",
      "design_ref": ".bloomery/specs/PARSER/REQUIREMENTS/README.md"
    }
  ]
}
```

JSON is intended for scripts and external workflow systems. It contains no
review status because Bloomery deliberately does not own that lifecycle.
