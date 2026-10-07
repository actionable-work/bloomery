---
id: DISK
name: Nix Disk Usage Reporting
tagline: Report the store closure size of the whole Bloomery tree or one addressed derivation.
description: |
  The disk feature owns the read-only `bloomery disk` command. It evaluates the
  workspace's Bloomery outputs, resolves the realized store closure for the full
  tree or a single addressed derivation, deduplicates shared store paths, and
  reports the resulting disk use in human-readable and JSON forms.
---

# `bloomery disk`

`bloomery disk` measures how much Nix store space Bloomery's build tree uses. It
is the reporting counterpart to [check](../CHECK/README.md): check realizes and
validates the tree, while disk measures the tree that is already realized. It
requires the shared root `flake.nix` preflight and `.bloomery/config.toml`.

## Design documents

- [Command usage](design/usage.md)
- [Measurement contract](design/measurement.md)
- [Category report](design/report.md)

## Responsibilities

- Measure the Bloomery packages and checks tree by default.
- Measure a single addressed derivation when one is supplied.
- Resolve realized store closures and deduplicate shared store paths.
- Report integer bytes in JSON and binary units in human output.
- Report unmeasured derivations without claiming a complete measurement.

## Boundaries

Disk is read-only. It does not build, substitute, realize, or repair anything,
and it does not mutate configuration, source, lockfiles, or the requirement
model. It evaluates only the workspace flake outputs it measures and does not
run the check formatter or static validation. Command syntax belongs to
[INTERFACE](../INTERFACE/README.md); the shared JSON contract lives in the
[CLI interface output design](../INTERFACE/design/output.md).
