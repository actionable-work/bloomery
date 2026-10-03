# Routes and rendering

The `bloomery-ui` library registers the home page and the documentation pages on
a Topcoat router. Documentation pages are reachable through their short routes
and `/docs/<slug>` routes. Shared components provide the site layout, navigation,
article rendering, and page-specific presentation.

The UI consumes the page catalog and Markdown renderer from `bloomery-content`;
it does not own the authored page content. CSS, logo, and favicon assets are
colocated with the UI crate and registered through Topcoat's asset API. The
server attaches a native asset bundle when one is available.
