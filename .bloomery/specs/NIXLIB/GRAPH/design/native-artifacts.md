# Build-script native artifacts

A build script may produce native link inputs under `OUT_DIR`, such as static
archives (`.a`) and shared objects (`.so`, `.dylib`). The crate builder installs
them into the crate's flat library directory before that directory is archived.
Native artifacts are archive-only: the crate output never exposes a separate
uncompressed native directory.

## Installation

After the build script runs, the crate builder copies every native static and
shared library found under `OUT_DIR` into `$out/lib`, preserving each library's
path relative to `OUT_DIR`. They sit alongside the crate's Rust libraries. The
builder then packs `$out/lib` into `$out/lib.tar.zst` and removes `$out/lib`, so
the archive is the only installed location for native artifacts.

## Propagation

Build-script `cargo:rustc-link-search` directives whose path is under `OUT_DIR`
are rewritten before they are published in the crate metadata: the `OUT_DIR`
prefix is replaced with the dependent's build-local dependency directory, which
is the extraction root for the dependency archives. A dependent extracts the
archives into that directory before compiling, so the rewritten search path
resolves only after extraction. `cargo:rustc-link-lib` directives and
link-search directives that do not reference `OUT_DIR` are preserved. A
dependent links the installed native libraries without user-provided link
flags.

## Archive and closure

Installed native libraries are members of the single `lib.tar.zst` archive. The
crate publishes that archive in its `deps-closure` together with the transitive
dependency archives, so every consumer extracts the same native inputs. No
native artifact is exposed outside the archive.

## Links awareness

A crate that declares `links` and whose build script produces native libraries
propagates those libraries and its `DEP_<LINKS>_<KEY>` metadata to dependents by
default. No per-crate override is required to link a build-script dependency.

## Builder tests

The crate builder test suite covers native propagation directly:

- a build-script fixture that emits a static native library, consumed by a
  dependent that links it;
- a build-script fixture that emits a shared native library, consumed by a
  dependent that links it; and
- archive membership, `deps-closure` propagation of native libraries, and the
  absence of an uncompressed library directory in the crate output.
