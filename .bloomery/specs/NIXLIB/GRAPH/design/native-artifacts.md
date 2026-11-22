# Build-script native artifacts

A build script may produce native link inputs under `OUT_DIR`, such as static
archives (`.a`) and shared objects (`.so`, `.dylib`). The crate builder installs
them into the crate's compile-time archive, `meta.tar.zst` (see
[Crate archives and dependency staging](crate-archives.md)). Native artifacts
are archive-only: the crate output never exposes an uncompressed library
directory.

## Installation

After the build script runs, the crate builder copies every native static and
shared library found under `OUT_DIR` into `meta.tar.zst`, preserving each
library's path relative to `OUT_DIR`. Native libraries travel in the
compile-time archive because a dependent `rlib` compilation bundles static
native libraries named by propagated `-l` flags, so every staging mode needs
them. The archive is the only installed location for native artifacts.

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

Installed native libraries are members of the crate's `meta.tar.zst`. The crate
lists itself in its `deps-closure`, so every consumer that stages the crate
extracts the same native inputs. No native artifact is exposed outside the
archive.

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
- membership of native libraries in `meta.tar.zst`, the crate's presence in its
  own `deps-closure`, and the absence of an uncompressed library directory in
  the crate output.
