# Resolved domain model

The in-memory context is a resolved graph:

```text
Context
├── Configuration
├── Areas[*]
│   ├── Frontmatter
│   └── Features[*]
│       ├── Frontmatter
│       ├── DesignDocuments[*]
│       └── RequirementGroups[*]
│           └── Requirements[*]
└── EvidenceRegistry
    └── RequirementId → SourceLocation[*]
```

Core value types include:

- `MarkdownFrontmatter { id, name, tagline, description }`;
- `RequirementGroup { group, requirements }`;
- `Requirement { id, title, manual, design, ears }`;
- a typed `EarsStatement` enum for the six clause forms;
- `SourceLocation { file_path, line, column or span }`;
- structured design references containing a feature-relative path and optional
  anchor.

The EARS enum owns canonical statement generation. The context owns resolution
and identity alignment. Formatters consume these types without reparsing TOML,
Markdown, or source files.

Static scanning additionally produces test sites that pair a source location
with zero or more requirement references, as specified by the
[test discovery contract](../../SCANNING/design/test-discovery.md). Test sites are
a scanning output consumed by the CLI candidate projection and do not alter
requirement identity.

All IDs and paths retain their original spelling. Natural numeric ordering is a
presentation concern and must not mutate stored identifiers.
