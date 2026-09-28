In large monorepos, keeping package overrides centralized in a top-level Nix file leads to clutter. Bloomery supports colocated `overrides.nix` files placed directly next to any crate's `Cargo.toml`.

---

## Anatomy of an `overrides.nix`

An `overrides.nix` file can be an attribute set or a function taking `{ pkgs, lib, ... }`:

```nix
# packages/my-app/overrides.nix
{ pkgs, lib, ... }: {
  # Granular source filtering with lib.fileset
  fileset = lib.fileset.unions [
    ./src
    ./assets
    ./Cargo.toml
  ];

  # Custom compilation and linker flags
  rustcFlags = [ "-Ctarget-cpu=native" ];

  # Native system dependencies
  nativeBuildInputs = [ pkgs.pkg-config ];
  buildInputs = [ pkgs.openssl pkgs.zlib ];

  # Native framework asset aggregation
  assets = [ ./data.json ];
  assetDirs = [ "templates" "styles" ];

  # Build-time and runtime environment variables
  env = {
    APP_ASSETS_DIR = "./assets";
  };
}
```

---

## Native Framework Asset Pipeline

Bloomery automatically detects and forwards static assets for Rust web, GUI, and game engines (such as Topcoat, Dioxus, Bevy, and Leptos):

- **Default Directories**: Standard directories named `assets/`, `static/`, and `public/` in library or binary crates are automatically discovered and aggregated into `$out/bin/assets/`.
- **Custom Asset Directories (`assetDirs`)**: Declare custom directory names to collect from the crate source (e.g. `assetDirs = [ "templates" "fonts" ];`).
- **Explicit Assets (`assets`)**: Provide explicit path expressions or derivations to bundle (e.g. `assets = [ ./config.toml pkgs.my-data ];`).
- **Downstream Aggregation**: Libraries propagate their assets transitively; the final binary derivation merges upstream library assets with its own crate assets.

---

## Override Precedence & Merging

Overrides are merged automatically with the following precedence:

1. **Default built-in overrides** (e.g. `openssl-sys`, `zstd-sys`, `libsqlite3-sys`, `libgit2-sys`, `lz4-sys`, `bzip2-sys`, `prost-build`).
2. **Colocated `overrides.nix`** files discovered next to member `Cargo.toml` files.
3. **Flake-level explicit `overrides`** passed to `mkWorkspace` or `mkFlake`.

> [!TIP]
> Both hyphenated (`openssl-sys`) and underscored (`openssl_sys`) override identifiers are recognized and normalized automatically.
