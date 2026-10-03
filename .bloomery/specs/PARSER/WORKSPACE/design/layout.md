# Workspace layout

## Canonical roots

Bloomery's repository-local configuration and specification metadata live
below the repository's top-level `.bloomery/` directory:

```text
.bloomery/
├── config.toml                       # optional
└── specs/
    └── <AREA>/
        ├── README.md
        └── <FEATURE>/
            ├── README.md
            ├── design/
            │   └── **/*.md
            └── requirements/
                └── <GROUP>.toml       # at least one group file
```

The `config.toml` file is optional. When it is absent, Bloomery uses the
validated defaults described in the
[configuration file design](../../CONFIGURATION/design/config-file.md).

Every feature contains a `requirements/` directory with at least one grouped
TOML file. The checker rejects a feature whose requirements directory is absent
or empty.

## Hierarchy

- An area groups related responsibilities and owns their shared rules.
- A feature is a cohesive capability within one area.
- A feature README is its overview and default design target.
- Files under `design/` hold focused architecture, flow, state, protocol, and
  trade-off documents. Nested directories are allowed.
- Grouped TOML files under `requirements/` contain the EARS records owned by the
  feature.

In this repository, `CLI` describes command-facing capabilities and `PARSER`
contains shared input and evidence-processing capabilities. These names are
examples of the hierarchy, not reserved area names.

## Path rules

Directory names are identifiers, not display labels. A discovered path is
retained verbatim and compared by exact string equality. The CLI must not
silently lowercase, uppercase, trim, transliterate, or otherwise normalize an
area, feature, group, or document path.

Design references are relative to the owning feature directory. A reference
may point at the feature README or at any Markdown file below `design/`; an
optional `#anchor` identifies a heading in that file.
