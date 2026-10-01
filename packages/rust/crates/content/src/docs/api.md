Bloomery provides zero-boilerplate top-level constructors (`bloomery.mkFlake`), per-system workspace builders (`bloomery.lib.${system}.mkWorkspace`), and module integrations (`bloomery.flakeModules.default`).

---

## `mkFlake` Top-Level Constructor

`bloomery.mkFlake` accepts the standard workspace options plus flake wrapper parameters:

```nix
bloomery.mkFlake {
  inherit nixpkgs;
  systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ];
  root = ./.;

  # Optional extra flake outputs callback: { eachSystem, perSystemWorkspace }
  extraOutputs = { eachSystem, perSystemWorkspace }: {
    # Custom flake attributes
  };

  # ...All mkWorkspace options below can be passed directly
}
```

---

## `mkWorkspace` Complete Options Schema

The primary constructor for customized per-system workspaces is `bloomery.lib.${system}.mkWorkspace`:

```nix
bloomery.lib.${system}.mkWorkspace {
  # Top-level required workspace root path
  root = ./.;

  # ── Source & Files ──────────────────────────────────
  source = {
    cargoToml = ./Cargo.toml;       # defaults to root + "/Cargo.toml"
    cargoLock = ./Cargo.lock;       # defaults to root + "/Cargo.lock"
    bloomeryLock = null;            # auto-detected (defaults to root + "/bloomery.lock")
    members = null;                 # null discovers all workspace members automatically
  };

  # ── Toolchain & Linker ──────────────────────────────
  toolchain = {
    rustc = pkgs.rustc;             # rustc compiler derivation
    clippy = pkgs.clippy;           # clippy-driver derivation
    cargo = pkgs.cargo;             # cargo derivation (for lock generation)
    linker = "lld";                 # "lld", "mold", "system", or null
    lld = pkgs.lld;                 # LLVM lld package
    mold = pkgs.mold;               # Mold linker package
    stdenv = pkgs.stdenv;           # base stdenv
  };

  # ── Profiles & Compilation ──────────────────────────
  profile = {
    optLevel = 3;                   # 0, 1, 2, 3, "s", "z"
    lto = "thin";                   # "fat", "thin", "off", "full", "none", "yes", "no", bool
    codegenUnits = 1;               # positive integer
    panic = "abort";                # "unwind", "abort"
    strip = true;                   # true, false, "symbols", "debuginfo", "none"
    targetCpu = null;               # e.g. "x86-64-v3", "native"
    debuginfo = null;               # bool, 0, 1, 2, "limited", "full", "none"
    overflowChecks = null;          # bool
    linker = null;                  # custom linker executable binary or path
    linkArgs = [];                  # extra linker arguments (-Clink-arg=...)
  };

  profileDev = {
    optLevel = 0;                   # dev profile optimization level
    lto = "off";                    # dev profile LTO setting
    codegenUnits = 256;             # parallel compilation units for fast dev builds
    debuginfo = 2;                  # full debug symbols for dev
  };

  profileName = "release";          # active profile name ("release" or "dev")

  # ── Dev Binary Apps ──────────────────────────────────
  createDevPackages = true;         # generate dev profile apps (<bin>:dev)

  # ── Compiler & Linker Flags ─────────────────────────
  flags = {
    rustc = [ "-Copt-level=3" ];    # base rustc flags applied to all crates
    test = [];                      # extra flags for test runner compilation
    clippy = [];                    # extra flags when running clippy-driver
    doc = [ "-Dwarnings" ];         # extra flags for rustdoc building
    doctest = [];                   # extra flags for doctest runner
  };

  # ── Crate Overrides ─────────────────────────────────
  overrides = {
    openssl-sys = {
      nativeBuildInputs = [ pkgs.pkg-config ];
      buildInputs = [ pkgs.openssl ];
      rustcFlags = [];
      rustdocFlags = [];
      env = {};
      features = null;             # override crate features (null uses resolved)
      fileset = null;              # lib.fileset for source filtering
      src = null;                  # custom source derivation or path
      profile = {};                # per-crate release profile overrides
      profileDev = {};             # per-crate dev profile overrides
      assets = [];                 # extra asset files/dirs to copy into $out/bin/assets
      assetDirs = [];              # custom asset directory names to collect
    };
  };

  # ── Development Shell ───────────────────────────────
  devShell = {
    enable = true;                  # export devShells.default
    packages = [];                  # extra shell utility packages
    shellHook = "";                 # bash setup hook
  };

  # ── Checks & CI ─────────────────────────────────────
  checks = {
    enable = true;                  # generate test, clippy, doc, doctest checks
    includePackageChecks = true;    # build final packages as CI checks
    throwOnOutOfDate = false;       # error evaluation if bloomery.lock is out of date
  };

  # ── Feature Resolution ──────────────────────────────
  features = {
    unify = true;                   # Cargo-style feature unification
    cratesIoIndex = null;           # custom crates.io index directory
  };
}
```

---

## `flakeModules.default` Schema

For `flake-parts` users, import `bloomery.flakeModules.default` and configure under `perSystem`:

```nix
perSystem = { pkgs, ... }: {
  bloomery.workspace = {
    root = ./.;
    profile = { optLevel = 3; lto = "thin"; };
  };
};
```

---

## Workspace Return Value

The evaluated workspace attribute set returned by `mkWorkspace` exposes:

- `packages`: Release derivations for binaries (`<name>`) and, when enabled, libraries (`<crate>:lib`), plus `default` when a binary exists.
- `apps`: Runnable release/dev binaries, documentation apps (`<crate>:doc`), `lock`, and `default` when a binary exists.
- `checks`: Comprehensive check suite (`name:test`, `name:clippy`, `name:doc`, `name:doctest`, `workspace:lock`).
- `devShell`: Preconfigured `mkShell` environment with Rust toolchain and build utilities.
- `crates`: Map of all individual `.rlib` derivations in the dependency DAG.
- `config`: Fully evaluated options configuration set.
