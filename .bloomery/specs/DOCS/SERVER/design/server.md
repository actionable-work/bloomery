# Server startup and assets

The `bloomery-docs` binary delegates to `bloomery-docs-server`. The server
library reads the `PORT` environment variable as a port number and defaults to
`8080` when it is missing or cannot be parsed. It builds a Topcoat router with
the pages registered by `bloomery-ui`, then binds a TCP listener to
`127.0.0.1` on the selected port.

At startup, the server attempts to load a native Topcoat asset bundle. When
available, it attaches the bundle to the router. When unavailable, it emits a
warning and continues serving the registered routes. HTTP startup and asset
serving are separate from page content and presentation contracts.
