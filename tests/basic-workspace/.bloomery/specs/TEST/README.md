---
id: TEST
name: Nix Check Fixture
tagline: A valid traceability workspace for exercising the Bloomery check.
description: |
  This area provides a small valid Bloomery workspace for integration
  testing the Nix-generated bloomery:check output.
---

# Nix Check Fixture

The fixture contains valid Bloomery metadata and intentionally has no local
`bloomery` executable. Its Nix check must use the executable supplied by the
Bloomery flake input.
