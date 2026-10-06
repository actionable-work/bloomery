# Template catalog and content

## Catalog layout

Templates are seeded from the repository's top-level `templates/` directory.
Each immediate child directory is a template whose directory name is the
template name. Init bundles this catalog and resolves `--template NAME` by
exact directory-name match. The catalog contains `basic`, `axum`, and
`topcoat`, and `basic` is the default.

Template sources are scaffolding inputs, not workspace members. They are
outside the configured scanner targets and carry no requirement-evidence
metadata.

## Project substitution

Template files may contain the `{{project}}` placeholder. Init replaces every
occurrence with the derived project name before writing. The project name is
the target directory's final path component normalized to lowercase snake_case,
so it is valid as both a Cargo package name and a Rust identifier.

## Common files

Every template provides the same bootstrap files:

- `flake.nix` wires `bloomery.mkFlake` with a `nixpkgs` input and a `bloomery`
  input pointing at `github:actionable-work/bloomery`, passing `root = ./.`.
  The generated workspace inherits the default development shell, including the
  Bloomery CLI.
- `Cargo.toml` declares a `[workspace]` with `resolver = "2"` and the
  template's member crates.
- `.bloomery/config.toml` does not override the scanner settings. Generated
  workspaces rely on the [documented default scanner paths](../../../PARSER/CONFIGURATION/design/schema-catalog.md#spec-and-scanner-tables),
  so the template crates live under `packages/rust/` and the configured
  defaults discover them without per-template overrides.
- `.bloomery/specs/` contains a limited area, feature, design, and requirement
  skeleton matching the template's crates.
- `.gitignore` ignores `target/`, `result`, `result-*`, and `.direnv/`.
- `.envrc` contains `use flake` for direnv.

Every template is a multi-crate workspace. Binaries are thin entry points that
delegate to library crates holding the application logic.

## Seeded specifications

Each template seeds a minimal specification tree: one area README, one or two
feature READMEs, one design document per feature, and one grouped requirements
file per feature. Seeded requirement records are `manual = false` and describe
the generated library behavior. Each seeded requirement is backed by a unit
test in the matching library crate tagged with
`#[cfg_attr(any(), bloomery("ID"))]`, so a fresh workspace passes structural
validation and coverage with no proc-macro dependency. Seeded IDs follow the
normal area-feature-group-sequence scheme and seed paths match the template's
crate layout.

## Basic workspace

The `basic` template is a multi-crate Rust workspace:

- `packages/rust/{{project}}_core` is the library holding the greeting logic.
- `packages/rust/{{project}}` is a thin binary that calls the library.

Its `APP/CORE` feature seeds one automated requirement for the greeting
behavior, backed by a tagged unit test in the core library.

## Axum server

The `axum` template is a multi-crate Rust workspace:

- `packages/rust/{{project}}_server` is the library holding the router and
  handlers,
  declaring `axum`, `tokio`, `serde`, and `serde_json` dependencies.
- `packages/rust/{{project}}` is a thin binary that starts the server.

Its `SERVER/HTTP` feature seeds automated requirements for the index and health
routes, backed by tagged unit tests in the server library.

## Topcoat website

The `topcoat` template is a multi-crate Rust workspace with a server/UI split:

- `packages/rust/{{project}}_ui` is the library holding pages, components, and
  static assets.
- `packages/rust/{{project}}_server` is the library that registers the UI pages
  onto the router and serves the site, depending on the UI crate.
- `packages/rust/{{project}}` is a thin binary that starts the server.

It seeds `SITE/UI` and `SITE/SERVER` features with automated requirements for
page registration and serving, backed by tagged unit tests in the UI and server
libraries. `topcoat` and `tokio` dependencies use the
features needed to render pages and serve the site.
