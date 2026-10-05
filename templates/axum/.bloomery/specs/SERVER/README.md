---
id: SERVER
name: Server Area
tagline: HTTP behavior for the generated Axum server.
description: |
  The server area owns the routes exposed by the generated Axum application.
---

# Server Area

The server is split into a library crate holding the router and handlers and a
thin binary crate that starts it.
