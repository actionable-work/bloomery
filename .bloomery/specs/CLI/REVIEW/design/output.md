# Review output

## Human-readable output

The default output presents the hierarchy as a tree and includes the title,
canonical EARS statement, and design path for each manual requirement. Area and
feature headings, requirement IDs, and the final count are highlighted with
terminal colors when supported; non-empty `NO_COLOR` and redirected output
disable ANSI styling.

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

`bloomery review --json` emits an array with stable field names and no ANSI
escape sequences:

```json
[
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
```

JSON is intended for scripts and external workflow systems. It contains no
review status because Bloomery deliberately does not own that lifecycle.
