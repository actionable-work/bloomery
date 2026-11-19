# Topcoat assets

Bloomery bundles Topcoat `asset!` declarations natively, replacing Topcoat's
own bundler step. The crate graph builds the Rust bundler for the build host
and uses it for artifact graphs that include `topcoat-asset`. Each declaration
embedded in a compiled artifact carries its
asset ID, declared path, declaring crate name, `CARGO_MANIFEST_DIR`, declaring
source file, and the `rename`, `extension`, `checksum`, and `content_type`
options. The bundler decodes all of them and follows Topcoat's path resolution:
absolute paths are used as-is, `./` and `../` paths start from the declaring
source file's directory, and other relative paths start from the manifest
directory.

## Asset index

A workspace or registry crate scans its compiled libraries for declarations
matching its crate name and manifest directory. Topcoat builds use
source-specific directories inside the sandbox so that dependencies with
the same Rust crate name remain distinguishable. The indexer copies each
resolved local file into
`$out/topcoat-assets/<asset-id>/<file-name>`. Indexing runs inside the crate
build, so paths into the crate sources and into the build-script `OUT_DIR`
both resolve; files a build script generates, such as a Tailwind stylesheet,
are indexed like checked-in files. Declarations that do not resolve at this
stage are left for the binary to report. Metadata-only variants do not index.

A crate derivation links each dependency's index entries into its own index,
so a binary sees the transitive closure. Different contents published under
one asset ID fail indexing or merging.

## Binary bundling

A binary derivation merges the indexes of its dependencies and its library
crate, then indexes the binary crate's own declarations from its sources and
build-script `OUT_DIR`. It publishes those own entries under
`$out/share/<bin>/topcoat-assets`. An optimized package copies the assets
installed by the compilation supplying its executable, including assets from
profile-use dependencies. BOLT training exposes the compilation's asset
directories, including configured directories, next to the instrumented
executable.

Every declaration in the executable is resolved by asset ID through the merged
index; file names never select an entry, so same-named files from different
crates bundle separately. A declaration without an index entry fails the build
with an error naming the declared path, the declaring crate and source file,
and the path it resolved to. Remote `http://` and `https://` declarations are
not downloaded inside the sandbox; they fail the build as unsupported, naming
the URL and crate, and the file must be vendored into the crate.

## Bundle format

Bundled files follow Topcoat's naming: the stem is the `rename` option or the
source file stem, the extension is the `extension` option or the source
extension. Both must be file name components. The first 16 hex digits of the
content's SHA-256 are inserted as
`<stem>-<hash>.<ext>`. An empty stem yields `<hash>.<ext>`, and a missing or
empty extension yields `<stem>.<hash>`. The content type is the
`content_type` option, otherwise it is guessed from the bundled file name with
the built-in table for common web types, falling back to
`application/octet-stream`. A `checksum` option must be `sha256:<hex>` and
match the content. A bundled name must identify one set of contents and one
content type.

`manifest.toml` has `version = 1` and one `[[assets]]` entry per asset ID, in
declaration order, with `id`, `file`, `hash`, and `content_type`. The bundle is
written to `$out/bin/assets`, where `AssetBundle::load` finds it next to the
executable, and to `$out/share/<bin>/assets`. Binaries without declarations
get no manifest.

## Tailwind

A crate derivation provides the Tailwind integration when it is
`topcoat-tailwind` with its `build` feature, or when a dependency provides it.
Every builder that runs a crate's build script — crate, binary, check, clippy,
doc, doctest, and test — puts nixpkgs `tailwindcss_4` on `PATH` and sets
`TAILWIND_CLI` to its executable when a build-script dependency provides the
integration. Other crates receive neither. An override's `env.TAILWIND_CLI`
takes precedence. The development shell carries the same CLI and variable when
any workspace member's build script needs it.

The CLI version is the one pinned by the workspace's nixpkgs input, not
Topcoat's download default. A build script selects it with:

```rust
topcoat::tailwind::BuildConfig::new()
    .executable_env("TAILWIND_CLI")
    .render()
    .expect("render the Tailwind stylesheet");
```

with `topcoat` as a build dependency carrying the `tailwind` feature.
`topcoat::tailwind::stylesheet!()` then declares the generated
`$OUT_DIR/tailwind.css`, which the asset index captures.
