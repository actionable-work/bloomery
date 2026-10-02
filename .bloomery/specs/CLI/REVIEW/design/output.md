# Review output

## Human-readable output

The default output presents the hierarchy as a tree and includes the title,
canonical EARS statement, and design path for each manual requirement:

```text
CLI
└── REQUIREMENTS
    └── AUTH
        └── [CLI-REQUIREMENTS-AUTH-001] Human review title
            While the account is locked, the login UI shall ...
            Design: .bloomery/specs/CLI/REQUIREMENTS/README.md

Total manual requirements requiring review: 1
```

## JSON output

`bloomery review --format json` emits an array with stable field names:

```json
[
  {
    "service": "CLI",
    "feature": "REQUIREMENTS",
    "group": "AUTH",
    "id": "CLI-REQUIREMENTS-AUTH-001",
    "title": "Human review title",
    "statement": "While the account is locked, the login UI shall ...",
    "design_ref": ".bloomery/specs/CLI/REQUIREMENTS/README.md"
  }
]
```

JSON is intended for scripts and external workflow systems. It contains no
review status because Bloomery deliberately does not own that lifecycle.
