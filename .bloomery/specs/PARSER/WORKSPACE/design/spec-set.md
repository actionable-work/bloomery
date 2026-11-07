# Derived specification set

Static reports expose `spec_set`, a version 1 manifest derived from every
requirement discovered under the configured specification root. Callers cannot
supply an active-ID allowlist. Hosted tooling manages repository files; Bloomery
reads that snapshot without consulting a registry or workflow service.

## Requirement revisions

Each report requirement carries a `revision`: SHA-256 of canonical JSON containing
`version = 1`, `id`, `title`, `manual`, typed `ears`, and `design` with its resolved
feature-relative `path` and optional `anchor`. Object keys are sorted; JSON is
compact UTF-8. The digest uses the `sha256:` prefix.

TOML formatting, comments, schema headers, record ordering and source positions
do not affect a record revision. Equivalent implicit and explicit design paths
resolve to the same revision. Record fields and resolved design references do
affect it. Referenced design document bytes remain bound by the specification
source identity, rather than the record revision. Revisions describe content;
they are not author-assigned sequence numbers or lifecycle approvals.

## Manifest

The manifest contains its `version`, `identity`, resolved `source_root`,
`source_identity`, `configuration` and `scope` identities, an `active` map from
every discovered requirement ID to its record revision, `bindings_identity`,
the report's full `completeness`, and `diagnostics_identity`. Bindings and sorted
diagnostics use the same canonical hashing rules as the report. The manifest
identity hashes its canonical payload with `identity` omitted.

The source identity retains all files and directories under the enabled root.
Configuration and scope prevent reuse across different validation settings;
binding identity includes locations and disabled or unresolved condition facts.
The enclosing report binds the manifest to the exact tree and parser/schema
contracts. Unrelated tree bytes can change the report without changing the
manifest. No manifest file is written into the tree it identifies.

A successful load produces the complete active map, including manual and
unbound requirements. Incomplete or failed discovery retains that map and
explicit stage states. Disabled specifications produce an empty map with the
disabled state and empty source identity. Invalid configuration, unreadable
tree content, malformed specifications or duplicate definitions produce no
manifest; the report retains their diagnostics and stage states. A partial
active map never masquerades as successful evidence.

CLI, filesystem and virtual-tree reports expose identical manifests. Static
revisions, bindings and completeness certify neither execution nor hosted
approval. Report/parser contracts are 8; source schema remains 1.
