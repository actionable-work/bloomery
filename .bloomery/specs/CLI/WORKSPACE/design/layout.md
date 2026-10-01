# Workspace layout

## Canonical roots

All Bloomery-owned files live below the repository's top-level `.bloomery/`
directory:

```text
.bloomery/
├── config.toml
└── specs/
    └── <SERVICE>/
        ├── README.md
        └── <FEATURE>/
            ├── README.md
            ├── design/
            │   └── **/*.md
            └── requirements/
                └── <GROUP>.toml       # at least one group file
```

Every feature contains a `requirements/` directory with at least one grouped
TOML file. The checker rejects a feature whose requirements directory is absent
or empty.

## Hierarchy

- A service is a domain boundary and owns cross-cutting invariants.
- A feature is a cohesive capability within one service.
- A feature README is its overview and default design target.
- Files under `design/` hold focused architecture, flow, state, protocol, and
  trade-off documents. Nested directories are allowed.
- Grouped TOML files under `requirements/` contain the EARS records owned by the
  feature.

The CLI service tree is `specs/CLI/`, with `WORKSPACE`, `CONFIGURATION`,
`REQUIREMENTS`, `SCANNING`, `CHECK`, `REVIEW`, and `IMPLEMENTATION` features.

## Path rules

Directory names are identifiers, not display labels. A discovered path is
retained verbatim and compared by exact string equality. The CLI must not
silently lowercase, uppercase, trim, transliterate, or otherwise normalize a
service, feature, group, or document path.

Design references are relative to the owning feature directory. A reference
may point at the feature README or at any Markdown file below `design/`; an
optional `#anchor` identifies a heading in that file.
