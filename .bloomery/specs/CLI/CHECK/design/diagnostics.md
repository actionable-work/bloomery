# Diagnostics

Diagnostics identify the failed invariant, explain the expected relationship,
and point to a repository-relative source location where possible. Output is
stable across runs: paths are normalized to repository-relative form, and
collections are sorted by area, feature, group, sequence, then source
location.

The diagnostic vocabulary includes:

| Code | Meaning |
| --- | --- |
| `DirectoryIdMismatch` | A directory and README `id` disagree. |
| `MissingDocument` | A required area or feature README is absent. |
| `MissingDesignFile` | An explicit design path cannot be resolved. |
| `DanglingDesignAnchor` | A requested Markdown heading anchor is absent. |
| `SpecIdPathMismatch` | An ID segment disagrees with its physical path. |
| `DuplicateSpecId` | An ID is declared more than once. |
| `OrphanTestReference` | Evidence names no declared requirement. |
| `IllegalManualAutomation` | Evidence references a manual requirement. |
| `MissingAutomatedTest` | An automated requirement has no evidence. |
| `ParseError` | Configuration, Markdown, TOML, source, or Nix input is invalid. |

The presentation resembles compiler diagnostics:

```text
ERROR [MissingAutomatedTest]:
  Requirement marked manual = false has 0 linked tests.
  --> .bloomery/specs/PARSER/REQUIREMENTS/requirements/AUTH.toml:12
```

Multiple independent failures are reported in one invocation. A diagnostic
does not trigger an attempted repair; the CLI is read-only.
