# Disk usage command

```text
bloomery disk [DERIVATION] [--scope SCOPE] [--system SYSTEM]... [--json]
bloomery disk tree [DERIVATION] [--scope SCOPE] [--system SYSTEM]... [--json]
```

`bloomery disk` reports the Nix store size of Bloomery's build tree. It is a
read-only measurement command: it evaluates the workspace flake, resolves
realized store paths, and prints totals. It never builds, substitutes, mutates
configuration, source, or lockfiles, and it creates no result links.

## Measurement modes

- With no `DERIVATION`, the command measures the
  [full tree](measurement.md#full-tree) for the selected systems.
- With a `DERIVATION`, the command measures only that derivation's closure.

## Scope

`--scope` selects which side of the full tree is measured:

| Scope | Measured outputs |
| --- | --- |
| `all` (default) | runtime packages and build checks |
| `runtime` | `packages.<system>.*` |
| `build` | `checks.<system>.*` |

`runtime` outputs are the deployable packages. `build` outputs are the check,
test, documentation, and clippy artifacts. Supplying a non-default scope with a
`DERIVATION` is a usage error.

A single-scope measurement reports the closure of that scope's outputs, so it
can include dependencies shared with the other scope. The combined `all` report
attributes each shared store path once, which is the runtime-versus-build split.

## Output modes

Plain `disk` prints the measured total, a line per measured scope, the selected
systems, and the count of unmeasured derivations. `disk tree` prints the same
measurement as an indented [category tree](report.md#rendering). Both modes
measure identically and share one JSON contract.

## Derivation addressing

`DERIVATION` accepts either form:

- A system-qualified flake output attribute path under `packages`, `checks`,
  `devShell`, or `formatter`, such as `packages.x86_64-linux.default` or
  `checks.x86_64-linux.my-crate:test`.
- A Nix store path or derivation path, such as `/nix/store/...-my-app` or
  `/nix/store/...-my-app.drv`. A derivation path is resolved to its realized
  output paths before measurement.

An argument that resolves to neither is a usage error. The `--system` option
selects the systems measured for a full-tree run and defaults to the host Nix
system. A derivation argument identifies its own target, so supplying
`--system` with a `DERIVATION` is a usage error.

## System scope

`--system` is repeatable and deduplicated. With no value the host Nix system is
measured. An explicitly requested system whose outputs cannot be evaluated is an
error rather than an implicit skip.

## Read-only measurement

The command requires the shared root `flake.nix` preflight and
`.bloomery/config.toml`. It disables lockfile writes and updates in every Nix
operation and does not depend on the requirement model or evidence scanners.
