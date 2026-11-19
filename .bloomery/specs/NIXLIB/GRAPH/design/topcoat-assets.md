# Topcoat assets

Bloomery bundles Topcoat `asset!` declarations natively, replacing Topcoat's
own bundler step. Each declaration embedded in a compiled artifact carries its
asset ID, declared path, declaring crate name, `CARGO_MANIFEST_DIR`, declaring
source file, and the `rename`, `extension`, `checksum`, and `content_type`
options. The bundler decodes all of them and follows Topcoat's path resolution:
absolute paths are used as-is, `./` and `../` paths start from the declaring
source file's directory, and other relative paths start from the manifest
directory.

## Asset index

A crate derivation scans its compiled library artifacts for declarations made
by its own crate name and copies each local file they resolve to into
`$out/topcoat-assets/<asset-id>/<file-name>`. Indexing runs inside the crate
build, so paths into the crate sources and into the build-script `OUT_DIR`
both resolve; files a build script generates, such as a Tailwind stylesheet,
are indexed like checked-in files. Declarations that do not resolve at this
stage are left for the binary to report. Metadata-only variants do not index.

A crate derivation links each dependency's index entries into its own index,
so a binary sees the transitive closure. Two different files published under
one asset ID fail the build.

## Binary bundling

A binary derivation merges the indexes of its dependencies and its library
crate, then indexes the binary crate's own declarations from its sources and
build-script `OUT_DIR`. It publishes those own entries under
`$out/share/<bin>/topcoat-assets`. An optimized package installs a prebuilt
executable, so it merges the index published by the release compilation it
optimizes instead of indexing.

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
extension, and the first 16 hex digits of the content's SHA-256 are inserted as
`<stem>-<hash>.<ext>`. An empty stem yields `<hash>.<ext>`, and a missing or
empty extension yields `<stem>.<hash>`. The content type is the
`content_type` option, otherwise it is guessed from the bundled file name with
Topcoat's table for common web types, falling back to
`application/octet-stream`. A `checksum` option must be `sha256:<hex>` and
match the content. One bundled file declared with two content types fails the
build.

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
