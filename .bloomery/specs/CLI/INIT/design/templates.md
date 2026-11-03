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
  The generated `nixpkgs` input follows the input declared by Bloomery, and
  `mkFlake` receives that shared input. The generated workspace inherits the
  default development shell, including the Bloomery CLI.
- `Cargo.toml` declares a `[workspace]` with `resolver = "2"`, the template's
  member crates, and shared dependencies under `[workspace.dependencies]`.
  Every shared dependency disables default features, and member manifests
  inherit dependencies with `workspace = true`.
- `.bloomery/config.toml` declares the current configuration format version
  and includes every key with a recommended default in Bloomery's
  [configuration schema catalog](../../../PARSER/CONFIGURATION/design/schema-catalog.md),
  using each catalogued value. A new recommendation is included by every
  template. Keys without a recommended default remain omitted. Recommended
  scanner paths explicitly discover template crates under `packages/rust/`.
- `.bloomery/specs/` contains a limited area, feature, design, and requirement
  skeleton matching the template's crates.
- `.gitignore` ignores `target/`, `result`, `result-*`, and `.direnv/`.
- `.envrc` contains `use flake` for direnv.

Every template is a multi-crate workspace. Binaries are thin entry points that
delegate to library crates holding the application logic.

## Library crates

Library crate package names use stable template-defined Rust identifiers:
`app_core` for the basic core library, `app_server` for server libraries, and
`app_ui` for the Topcoat UI library. Their directory names use the derived
project name. Binary package names use the derived project name.

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

## End-to-end validation

Every bundled template is validated through a freshly initialized workspace,
including project-name substitution, generated configuration, source files,
and lockfiles. The generated workspace must complete the default write-mode
`bloomery check` successfully with the Bloomery implementation under test. This
exercises the same formatting, workspace, dependency, code-health, and
specification checks that a user runs after initialization. The first
write-mode check may create the configured code-health baseline.

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
