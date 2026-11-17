# Routes and rendering

The `bloomery-ui` library registers the home page and the documentation pages on
a Topcoat router. Documentation pages are reachable through their short routes
and `/docs/<slug>` routes. Shared components provide the site layout, navigation,
article rendering, and page-specific presentation.

The UI consumes the page catalog and Markdown renderer from `bloomery-content`;
it does not own the authored page content. CSS, logo, and favicon assets are
colocated with the UI crate and registered through Topcoat's asset API. The
server attaches a native asset bundle when one is available.

## Browser behavior

The sidebar opens the selected guide and marks its link active. Search filters
the guide catalog and opens the selected result. Control/Command-K opens search
and focuses its input; Escape dismisses it. Native stylesheets and the brand
logo load through the asset routes in Chromium. The docs browser check launches
the packaged docs server and blocks optional remote fonts and GitHub requests.
