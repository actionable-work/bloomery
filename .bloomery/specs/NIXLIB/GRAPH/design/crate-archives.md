# Crate archives and dependency staging

A crate derivation publishes its compiled artifacts as zstd-compressed archives
instead of an uncompressed `lib/` directory. Consumers stage only the archives
their compilation needs. Archiving is an installation detail: crate, binary,
test, documentation, and clippy outcomes are the same as from uncompressed
libraries.

## Archive layout

Each crate output holds two archives, both extracted into the same flat
directory:

| Archive | Members |
| --- | --- |
| `meta.tar.zst` | `.rmeta` files, the proc-macro dynamic library, and build-script native libraries |
| `lib.tar.zst` | `.rlib` files and linked crate-type outputs (`cdylib`, `staticlib`) |

`meta.tar.zst` holds everything a non-linking compilation reads. `lib.tar.zst`
holds what only linking reads. `rustc` writes into a build-local directory and
the archives are packed from there, so the crate output receives only the
finished archives. A proc-macro crate publishes no
`lib.tar.zst`.

The crate metadata script publishes each archive path (`DEP_META_ARCHIVE`,
`DEP_LIB_ARCHIVE`) and the primary member for each mode: `DEP_META_NAME` (the
`.rmeta`, or the proc-macro library) and `DEP_LIB_NAME` (the `.rlib`, or the
proc-macro library).

## Reproducible archives

Archives are byte-reproducible so that independent builds of the same
derivation produce identical outputs, and substituted outputs match local
rebuilds. Members are sorted by name, every member has mtime `@1` and numeric
owner and group `0`, the tar format is GNU, and compression is single-threaded
zstd at level 3.

## Dependency closure

`nix-support/deps-closure` lists, one per line and sorted, the crate output
paths a consumer must stage alongside the crate: the crate itself and the union
of its dependencies' closures. A proc macro is self-contained at its use sites,
so its closure lists only itself. Ordinary library directories supplied as
dependencies appear as their own entry. A crate output references only the
crate outputs in its closure.

## Staging

Every Rust builder stages dependencies into a build-local directory before
invoking `rustc`. Staging takes the union of the direct dependencies' closures,
extracts each unique archive once, and runs extractions in parallel bounded by
the build cores. Only direct dependencies are bound with `--extern`; transitive
artifacts resolve through `-L dependency`.

Staging has two modes:

- `meta` stages only `meta.tar.zst` and binds each direct dependency's
  `DEP_META_NAME`. Rlib compilation of a crate without linked crate types,
  check, clippy, and documentation use it.
- `link` also stages `lib.tar.zst` and binds `DEP_LIB_NAME`. Binaries, tests,
  doctests, build scripts, proc macros, and crates with `cdylib` or
  `staticlib` outputs use it.

An `rlib` compiled against `.rmeta` metadata is valid for linking because the
metadata and the library come from the same `rustc` invocation.

## Builder tests

The builder test suite covers archives with real builds of a dependency-diamond
fixture:

- the same crate built twice as separate derivations produces byte-identical
  archives with identical unpacked contents;
- archive members are sorted with mtime `@1` and owner `0/0`;
- `link` staging produces the same file set as extracting every closure archive
  of every direct dependency;
- `meta` staging produces no `.rlib`; and
- a crate output references only the crate outputs in its closure and its
  metadata names no build-local path.
