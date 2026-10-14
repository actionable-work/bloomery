# Documentation consistency

Authored documentation drifts from the specification when features, commands,
and configuration change. This design makes documentation coverage and
references explicit and validates them statically.

## Documentation manifest

`.bloomery/docs.manifest.toml` is the repository-local documentation coverage
manifest. It declares the
`roots` globs that enumerate authored documentation surfaces and one
`[[surfaces]]` entry per surface. A surface entry names its repository-relative
`path` and the specification `features` it documents.

Both `roots` and `path` are repository-root-relative, so the manifest addresses
the same files regardless of where it is stored.

```toml
roots = [
  "README.md",
  "packages/rust/libs/docs/content/src/docs/*.md",
]

[[surfaces]]
path = "README.md"
features = ["NIXLIB/FLAKE", "CLI/INTERFACE"]

[[surfaces]]
path = "packages/rust/libs/docs/content/src/docs/api.md"
features = ["NIXLIB/FLAKE"]
```

The manifest is repository data, not shared Bloomery configuration. It lives
under `.bloomery/` with the rest of the repository's Bloomery metadata and is
read only by the repository's documentation consistency check.

## Coverage

Coverage is feature-level: a feature is documented when at least one surface
names it, and requirements and design documents are covered through their
feature.

The workspace is coverage-complete when:

- every root match has exactly one manifest entry;
- every manifest surface path exists;
- every feature named by a surface exists in the specification tree;
- every feature in the specification tree is named by at least one surface.

A root match without a manifest entry is an unregistered surface. A registered
path without a file is a missing surface. A surface naming a feature absent
from the tree is unknown. A tree feature named by no surface is undocumented.

## Links

Markdown links inside a registered surface that target a repository-relative
path are resolved against the repository. A target that does not exist is a
dangling link; a fragment that names no heading in the target is a dangling
anchor. External URLs are never fetched.

## Excerpts

Fenced code blocks may carry a Bloomery role as their second info-string token,
after the language:

````
```toml bloomery-config
[build]
profileName = "release"
```
````

A `bloomery-config` block is parsed as TOML and every leaf key path must exist
in the
[configuration schema catalog](../../../PARSER/CONFIGURATION/design/schema-catalog.md).
A `bloomery-cli` block is tokenized into `bloomery` command lines and every
command path must exist in the CLI command catalog described by
[CLI/INTERFACE](../../../CLI/INTERFACE/design/commands.md). Blocks without a
Bloomery role are illustrative and are not validated. The content renderer
displays only the language token, so a role token does not change how a block
is labeled.

## Check integration

A repository-local check in the root flake's check suite runs manifest,
coverage, link, and excerpt validation, so `bloomery check` executes it like
any other Nix check. It reads the specification tree and the configuration and
command catalogs, reports violations as check failures, and leaves the
workspace unchanged. The check is specific to this repository and is not part
of the shared Bloomery CLI or static specification pipeline.

## Diagnostics

These codes belong to the repository check and are not part of the shared
static diagnostic vocabulary.

| Code | Meaning |
| --- | --- |
| `UnregisteredDocumentationSurface` | A documentation root match has no manifest entry. |
| `MissingDocumentationSurface` | A manifest surface path does not exist. |
| `UnknownDocumentedFeature` | A manifest surface names a feature absent from the tree. |
| `UndocumentedFeature` | A specification feature is named by no surface. |
| `DanglingDocumentationLink` | A surface link target does not exist. |
| `DanglingDocumentationAnchor` | A surface link fragment names no heading in its target. |
| `MalformedConfigExcerpt` | A tagged configuration excerpt is not valid TOML. |
| `UnknownConfigKeyInDocumentation` | A `bloomery-config` excerpt uses a key outside the schema catalog. |
| `UnknownCommandInDocumentation` | A `bloomery-cli` excerpt uses a command path outside the command catalog. |
