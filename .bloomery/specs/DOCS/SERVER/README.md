---
id: SERVER
name: Documentation HTTP Server
tagline: Serve the Bloomery documentation UI on a configurable local port.
description: |
  The server feature owns startup of the `bloomery-docs` binary, registration of
  UI routes, optional Topcoat asset-bundle loading, and the local HTTP listener.
---

# Documentation Server

The `bloomery-docs` executable composes the UI router and serves it on the
loopback interface. It reads `PORT` from the environment and defaults to port
8080. If a native Topcoat asset bundle is available, the server attaches it;
absence of the bundle is reported without preventing startup.

## Design documents

- [Server startup and assets](design/server.md)
