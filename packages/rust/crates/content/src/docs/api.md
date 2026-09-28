The primary constructor for customized workspaces is `bloomery.lib.${system}.mkWorkspace`. It accepts a strongly-typed and categorized option set.

---

## Complete Options Schema

```nix
bloomery.lib.${system}.mkWorkspace {
  # Top-level required root path
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
    linker = "lld";                 # "lld" (fast default on Linux), "mold", or null
    lld = pkgs.lld;                 # LLVM lld package
    mold = pkgs.mold;               # Mold linker package
    stdenv = pkgs.stdenv;           # base stdenv
  };

  # ── Compilation Profile ─────────────────────────────
  profile = {
    optLevel = 3;                   # 0, 1, 2, 3, "s", "z"
    lto = "thin";                   # "fat", "thin", "off", "full", or bool
    codegenUnits = 1;               # positive integer
    panic = "abort";                # "unwind", "abort"
    strip = true;                   # true, false, "symbols", "debuginfo"
    targetCpu = null;               # e.g. "x86-64-v3", "native"
    debuginfo = null;               # bool, 0, 1, 2, "limited", "full"
    overflowChecks = null;          # bool
    linker = null;                  # custom linker executable
    linkArgs = [];                  # extra linker flags
  };

  # ── Compiler & Linker Flags ─────────────────────────
  flags = {
    rustc = [ "-Copt-level=3" ];    # base flags applied to all crates
    test = [];                      # extra flags for test runners
    clippy = [];                    # extra flags for clippy-driver
    doc = [ "-Dwarnings" ];         # extra flags for rustdoc
    doctest = [];                   # extra flags for doctest runner
  };

  # ── Crate Overrides ─────────────────────────────────
  overrides = {
    openssl-sys = {
      nativeBuildInputs = [ pkgs.pkg-config ];
      buildInputs = [ pkgs.openssl ];
      rustcFlags = [];
      env = {};
      features = null;
      assets = [];                  # extra asset files/dirs to copy into $out/bin/assets
      assetDirs = [];               # custom asset directory names to collect
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
    throwOnOutOfDate = false;       # error if bloomery.lock is behind Cargo.lock
  };

  # ── Features ────────────────────────────────────────
  features = {
    unify = true;                   # Cargo-style feature unification
    cratesIoIndex = null;           # custom crates.io index directory
  };
}
```

---

## Workspace Return Value

The evaluated workspace attrset exposes:

- `packages`: Derivation attribute set for all workspace binaries and libraries.
- `apps`: Runnable apps (`apps.${system}.<name>`) with entry points.
- `checks`: Comprehensive check suite (`name:test`, `name:clippy`, `name:doc`, `name:doctest`, `name:bin`, `name:lib`, `workspace:lock`).
- `devShell`: Configured `mkShell` environment.
- `crates`: Map of all individual `.rlib` derivations in the dependency DAG.
- `config`: Fully evaluated options configuration.
