---
id: BUILD
name: Production Build Materializer
tagline: Realize the exact production package derivations for a workspace.
description: |
  The build feature owns the `bloomery build` command. It evaluates the
  workspace packages output, selects production derivations, realizes them in
  one batched Nix invocation, and reports each package's output store path. By
  default it does not run the check suite; `--check` gates realization on the
  default check suite passing first.
---

# `bloomery build`

`bloomery build` is the production materialization entry point. It requires the
shared root `flake.nix` preflight and `.bloomery/config.toml`. It evaluates the
selected systems' `packages.<system>` outputs, resolves each selected name to
its production derivation, and realizes the selection in one batched Nix
invocation. The production derivation for a binary is the optimized variant
when `[optimize]` enables it and the release build otherwise; dev-profile
derivations are never built. Build is independent of specification management:
it never loads the requirement model, so it runs whether or not a specification
tree exists.

Build is the counterpart to [check](../CHECK/README.md): check provides fast
workspace verification, while build produces the deployable package
derivations that `nix build .#packages.<system>.<name>` would produce. Passing
`--check` runs the default check suite as a gate before realizing packages, so
a single command can verify and build.

## Design documents

- [Command usage](design/command.md)
- [Production realization](design/execution.md)

## Responsibilities

- Require the shared root-flake preflight and `.bloomery/config.toml`.
- Run without loading the requirement model or requiring a specification tree.
- Select production packages by name or glob for the selected systems.
- Evaluate each selected system's packages catalog once.
- Realize the selected production derivations in one batched Nix invocation.
- Materialize the exact exported package derivation, including an optimized
  binary when `[optimize]` enables it.
- Report each package's output store path and per-package outcome.
- Gate realization on the default check suite when `--check` is selected.
- Leave source, configuration, and lockfiles unchanged.

## Boundaries

Build evaluates the workspace flake and realizes production package
derivations. By default it does not run `nix fmt`, static specification
validation, tests, clippy, documentation, or package checks; `--check` runs the
default check suite as a prerequisite and skips realization when it does not
pass. When specification management is disabled, that gate contains no static
specification checks and proceeds on the formatter and Nix checks. It creates no result links and
does not retain a run store; failures name the failed derivation path so a user
can retrieve its log with `nix log`. The `packages` output shape is owned by
[NIXLIB/FLAKE](../../NIXLIB/FLAKE/design/output-contract.md), and the
optimization pipeline that supplies an optimized package is owned by
[NIXLIB/OPTIMIZE](../../NIXLIB/OPTIMIZE/design/optimization.md). Command syntax
belongs to [INTERFACE](../INTERFACE/README.md).
