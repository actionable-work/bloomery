---
id: DOCS
name: Bloomery Documentation Area
tagline: Publish Bloomery guides and API documentation through a searchable web application.
description: |
  The DOCS area owns the documentation content catalog, web presentation, the
  server that exposes the Bloomery documentation site, and the consistency
  validation that aligns authored documentation and the README with the living
  specification. It separates static authored content from page rendering,
  server startup, and documentation drift detection.
---

# DOCS Area

The documentation site is implemented by the `bloomery-content` and
`bloomery-ui` libraries and the `bloomery-docs` server binary. This area
specifies their public responsibilities without coupling content authoring to
HTTP startup. It also owns the coverage manifest and static validation that
keep authored documentation aligned with the specification tree.

## Features

| Feature | Responsibility | Design |
| --- | --- | --- |
| [Content](CONTENT/README.md) | Page metadata, Markdown content, and catalog lookups | [page catalog](CONTENT/design/catalog.md) |
| [UI](UI/README.md) | Routes, shared presentation, and static assets | [routes and rendering](UI/design/routes-and-rendering.md) |
| [Server](SERVER/README.md) | HTTP server startup and asset-bundle integration | [server](SERVER/design/server.md) |
| [Consistency](CONSISTENCY/README.md) | Documentation coverage, links, and excerpt validation | [consistency validation](CONSISTENCY/design/consistency.md) |
