# TODO: native Topcoat asset bundling and Tailwind (WIP)

Branch `ccr-15645265-ux1124`. This is **work in progress**: implemented and
partly validated, but not yet reviewed or fully checked. It is pushed only
because the quota ran out. Delete this file when the work is done.

## Why

Hammerworks Platform P2.10 needs Topcoat's Tailwind stylesheet in the binary's
asset bundle. The old `lib/builders/topcoat-bundler.py` did three things that
broke this:

- it only looked at crate `assets`, `static` and `public` directories;
- it matched declarations by **file name**;
- it skipped declarations it could not resolve, without any error.

A build-script-generated `$OUT_DIR/tailwind.css` therefore never reached the
derivation. A probe project built fine, but its manifest held only the control
asset.

## What is implemented (uncommitted until now; see `git show HEAD`)

- **Asset index.** Each full crate build scans its compiled library for
  `TOPCOAT_ASSET` declarations. It decodes the topcoat-asset 0.10 encoding and
  resolves each path the way Topcoat does. Each resolved file goes to
  `$out/topcoat-assets/<asset-id>/`. This runs while the build-script
  `OUT_DIR` still exists, and the index is forwarded to dependents.
- **Binary bundling.**
  - The binary merges its dependencies' indexes and resolves each declaration
    **by asset ID**.
  - Bundled files are named `<stem>-<16 hex of sha256>.<ext>`, as in
    Topcoat's own bundler.
  - The `sha256:` checksum and content-type options are honoured.
  - The manifest is version 1.
- **Failures.** These now fail the build:
  - a declaration that cannot be resolved, with an error naming the path,
    the crate and the source file;
  - a remote `http(s)` declaration (unsupported; vendor the file instead);
  - one asset ID with two different files, or one file with two content
    types.
- **Tailwind provisioning.**
  - Builders that run a build script add `pkgs.buildPackages.tailwindcss_4`
    and set `TAILWIND_CLI` when a build dependency is `topcoat-tailwind`
    with the `build` or `default` feature (`lib/builders/tailwind.nix`).
  - A `TAILWIND_CLI` set in an override's `env` wins.
  - The dev shell gets the same CLI and variable.
- **Optimized binaries** reuse the asset index of the release build they
  optimize.
- **Specs:** new requirements `NIXLIB-GRAPH-ASSETS-001…018` in
  `.bloomery/specs/NIXLIB/GRAPH/requirements/ASSETS.toml`, the new design
  `design/topcoat-assets.md`, and updates to `derivations.md` and
  `output-contract.md`.
- **Tests:**
  - root checks `nix/checks/topcoat-bundler-{failures,naming}.nix` and
    `topcoat-bundler-test.py`;
  - unit tests in `lib/builders/builders.test.nix`;
  - a new fixture sub-flake, `tests/topcoat-assets-workspace`. Two crates
    ship the same `assets/site.css`, a Tailwind build script renders the
    stylesheet, and a serving check fetches the bundled CSS and finds
    generated utilities.

### Validation so far

These targeted runs passed:

- `static:*`
- `core:treefmt-check`
- `core:topcoat-bundler-*`
- `core:unit-tests`
- `topcoat-assets-workspace:checks`, including end-to-end serving

The full `nix run .#bloomery:dev -- check` has **not** completed yet. Earlier
runs died when the disk filled up. A rerun after `nix store gc` was stopped
partway because the quota ran out.

## Remaining

1. Run the full `nix run .#bloomery:dev -- check` and fix anything it finds.
   Every crate derivation changed, so expect long rebuilds and keep 20 GB or
   more of disk free.
2. Review the behaviour changes:
   - the basename lookup is gone for Topcoat declarations;
   - binaries with declarations always get a manifest;
   - assets that registry crates declare in their own source (for example
     topcoat-runtime's `browser/dist/index.js`) now resolve instead of being
     skipped;
   - `templates/topcoat` and `tests/template-topcoat` (topcoat 0.9) were not
     updated.
3. Review the `.bloomery/config.toml` test exception for
   `topcoat_assets_workspace`, which allows 10 requirement IDs because Nix
   evidence is counted per file.
4. Do an independent review, then commit with conventional commits and merge.
5. Then bump the Hammerworks pin and finish its Tailwind slice; see the
   Hammerworks branch `TODO.md`.

How a project enables Tailwind with this change (no `overrides.nix` needed):
- add `topcoat` with `features = ["tailwind"]` as both a dependency and a
  build-dependency (the build-dependency with `default-features = false`);
- in `build.rs`, call
  `topcoat::tailwind::BuildConfig::new().executable_env("TAILWIND_CLI").render()`;
- link the stylesheet with `topcoat::tailwind::stylesheet!()`;
- attach the bundle with `AssetBundle::load()` on the router.
