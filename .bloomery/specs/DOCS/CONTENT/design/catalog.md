# Documentation page catalog

The `bloomery-content` library exposes a static catalog of documentation pages.
Each page contains a slug, title, category, description, display order, and
embedded Markdown content. The catalog supports listing all pages, looking up a
page by slug, and grouping pages into categories for navigation.

The current pages are `overview`, `quickstart`, `architecture`, `profiles`,
`api`, `overrides`, `matrix`, and `benchmarks`. Their Markdown sources live in
`packages/rust/crates/content/src/docs/` and are included at compile time.

The same library provides Markdown-to-HTML rendering for the documentation UI.
Content data and rendering do not own HTTP routes or server startup; those
responsibilities belong to [DOCS/UI](../../UI/README.md) and
[DOCS/SERVER](../../SERVER/README.md).
