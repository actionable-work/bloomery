# Validation pipeline

The command runs ordered passes so each check operates on a known model:

1. **Configuration ingestion** — parse `.bloomery/config.toml`, resolve roots,
   and validate scanner settings.
2. **Workspace and frontmatter** — discover area and feature directories,
   require their READMEs and non-empty `requirements/` directories, and compare
   frontmatter IDs with directory names.
3. **Requirement and design linkage** — parse grouped TOML records, validate
   EARS fields and IDs, enforce uniqueness, and resolve design files and
   anchors.
4. **Static evidence extraction** — run enabled Rust, Playwright, and Nix
   scanners and merge their references.
5. **Relational verification** — compare declared requirements and discovered
   references.
6. **Diagnostic reporting** — emit all deterministic diagnostics and select the
   process exit code.

Let:

```text
S        = all declared requirements
S_auto   = requirements with manual = false
S_manual = requirements with manual = true
T        = all unique statically discovered IDs
```

The relational invariants are:

```text
T ∖ S         = ∅   no test references an unknown requirement
T ∩ S_manual  = ∅   manual requirements have no automated references
S_auto ∖ T    = ∅   every automated requirement has evidence
```

The workspace pass requires every feature to provide architecture documents and
at least one grouped requirement file before the requirement and relational
passes run.
