---
id: CONTENT
name: Documentation Content Catalog
tagline: Define the published documentation pages and their metadata.
description: |
  The content feature owns the static documentation catalog, page metadata,
  Markdown source inclusion, category grouping, slug lookup, and Markdown-to-HTML
  rendering supplied by the `bloomery-content` library.
---

# Documentation Content

The content catalog separates authored Markdown from page routing and UI. Each
published page has a stable slug and presentation metadata; Markdown files are
embedded from the content crate at compile time.

## Design documents

- [Page catalog and content rendering](design/catalog.md)
