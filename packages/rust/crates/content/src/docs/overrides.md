# Colocated Overrides (`overrides.nix`)

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

  # Build-time and runtime environment variables
  env = {
    APP_ASSETS_DIR = "./assets";
  };
}
```

---

## Override Precedence & Merging

Overrides are merged automatically with the following precedence:

1. **Default built-in overrides** (e.g. `openssl-sys`, `zstd-sys`, `libsqlite3-sys`, `libgit2-sys`, `lz4-sys`, `bzip2-sys`, `prost-build`).
2. **Colocated `overrides.nix`** files discovered next to member `Cargo.toml` files.
3. **Flake-level explicit `overrides`** passed to `mkWorkspace` or `mkFlake`.

> [!TIP]
> Both hyphenated (`openssl-sys`) and underscored (`openssl_sys`) override identifiers are recognized and normalized automatically.
