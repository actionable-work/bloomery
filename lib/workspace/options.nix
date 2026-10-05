{
  pkgs,
  lib ? pkgs.lib,
}: let
  types = lib.types;
  profileTypes = import ../profile/types.nix {inherit lib;};

  # Submodule for test-only crate overrides. These inputs are overlaid on the
  # test builder only; they never participate in production compilation or
  # packaging.
  testOverrideOptionModule = {
    options = {
      nativeBuildInputs = lib.mkOption {
        type = types.listOf types.package;
        default = [];
        description = "Build-time native tools available to test compilation and execution only.";
      };
      buildInputs = lib.mkOption {
        type = types.listOf types.package;
        default = [];
        description = "Target native libraries available to test compilation and execution only.";
      };
      env = lib.mkOption {
        type = types.attrsOf types.str;
        default = {};
        description = "Environment variables set while compiling and running tests only.";
      };
      fileset = lib.mkOption {
        type = types.nullOr types.raw;
        default = null;
        description = "Additive fixture fileset included in test sources only.";
      };
    };
  };

  # Submodule for individual crate overrides
  overrideOptionModule = {
    options = {
      nativeBuildInputs = lib.mkOption {
        type = types.listOf types.package;
        default = [];
        description = "Build-time native tools/compilers (added to nativeBuildInputs).";
      };
      buildInputs = lib.mkOption {
        type = types.listOf types.package;
        default = [];
        description = "Target native libraries (added to buildInputs / C link paths).";
      };
      rustcFlags = lib.mkOption {
        type = types.listOf types.str;
        default = [];
        description = "Additional rustc compiler flags for this specific crate.";
      };
      rustdocFlags = lib.mkOption {
        type = types.listOf types.str;
        default = [];
        description = "Additional rustdoc flags for this specific crate.";
      };
      env = lib.mkOption {
        type = types.attrsOf types.str;
        default = {};
        description = "Environment variables set during crate compilation.";
      };
      features = lib.mkOption {
        type = types.nullOr (types.listOf types.str);
        default = null;
        description = "Override active features for this crate (null to use resolved features).";
      };
      fileset = lib.mkOption {
        type = types.nullOr types.raw;
        default = null;
        description = "Custom lib.fileset for this workspace crate.";
      };
      src = lib.mkOption {
        type = types.nullOr (types.either types.path types.package);
        default = null;
        description = "Custom source derivation or path for this workspace crate.";
      };
      profile = lib.mkOption {
        type = types.attrsOf types.anything;
        default = {};
        description = "Per-crate binary profile override settings.";
      };
      profileDev = lib.mkOption {
        type = types.attrsOf types.anything;
        default = {};
        description = "Per-crate binary dev profile override settings.";
      };
      assets = lib.mkOption {
        type = types.listOf (types.either types.path types.package);
        default = [];
        description = "Additional asset file or directory paths to bundle alongside binaries.";
      };
      assetDirs = lib.mkOption {
        type = types.listOf types.str;
        default = [];
        description = "Custom asset directory names to collect from the crate source (in addition to assets, static, public).";
      };
      test = lib.mkOption {
        type = types.submodule testOverrideOptionModule;
        default = {};
        description = "Test-only overrides (native inputs, environment, additive fixture fileset).";
      };
    };
  };

  # Main workspace option declarations grouped by category
  workspaceOptionModule = {config, ...}: {
    options = {
      # Top-level required setting
      root = lib.mkOption {
        type = types.path;
        description = "Path to the Rust workspace root directory (containing Cargo.toml).";
      };

      # ── Source & Files ──────────────────────────────────────────────────────
      source = {
        cargoToml = lib.mkOption {
          type = types.path;
          default = config.root + "/Cargo.toml";
          defaultText = "root + \"/Cargo.toml\"";
          description = "Path to workspace Cargo.toml.";
        };
        cargoLock = lib.mkOption {
          type = types.path;
          default = config.root + "/Cargo.lock";
          defaultText = "root + \"/Cargo.lock\"";
          description = "Path to Cargo.lock.";
        };
        bloomeryLock = lib.mkOption {
          type = types.nullOr types.path;
          default =
            if builtins.pathExists (config.root + "/bloomery.lock")
            then config.root + "/bloomery.lock"
            else null;
          defaultText = "root + \"/bloomery.lock\" (if exists)";
          description = "Path to bloomery.lock manifest (auto-detected if present).";
        };
        members = lib.mkOption {
          type = types.nullOr (types.listOf types.str);
          default = null;
          description = "Subset of workspace member crate names to build (null builds all).";
        };
      };

      # ── Toolchain & Linker ───────────────────────────────────────────────────
      toolchain = {
        rustc = lib.mkOption {
          type = types.package;
          default = pkgs.rustc;
          defaultText = "pkgs.rustc";
          description = "Rust compiler package.";
        };
        clippy = lib.mkOption {
          type = types.package;
          default = pkgs.clippy;
          defaultText = "pkgs.clippy";
          description = "Clippy driver package.";
        };
        cargo = lib.mkOption {
          type = types.package;
          default = pkgs.cargo;
          defaultText = "pkgs.cargo";
          description = "Cargo package (used for lock generation).";
        };
        linker = lib.mkOption {
          type = types.nullOr (types.enum ["lld" "mold" "system"]);
          default =
            if pkgs.stdenv.hostPlatform.isLinux
            then "lld"
            else null;
          defaultText = "if stdenv.hostPlatform.isLinux then \"lld\" else null";
          description = "Linker to use for binaries, test runners, and build scripts (\"lld\", \"mold\", or null for system default).";
        };
        lld = lib.mkOption {
          type = types.package;
          default = pkgs.lld;
          defaultText = "pkgs.lld";
          description = "LLVM lld package.";
        };
        mold = lib.mkOption {
          type = types.package;
          default = pkgs.mold;
          defaultText = "pkgs.mold";
          description = "Mold linker package.";
        };
        stdenv = lib.mkOption {
          type = types.package;
          default = pkgs.stdenv;
          defaultText = "pkgs.stdenv";
          description = "Standard build environment.";
        };
      };

      # ── Profiles & Compilation ───────────────────────────────────────────────
      profile = lib.mkOption {
        type = profileTypes.profileType;
        default = {};
        description = "Workspace binary compilation profile settings (optLevel, lto, codegenUnits, panic, strip, etc.).";
      };

      profileDev = lib.mkOption {
        type = profileTypes.profileType;
        default = {};
        description = "Workspace binary dev compilation profile settings (defaults to optLevel=0, lto=off, codegenUnits=256, debuginfo=2).";
      };

      profileName = lib.mkOption {
        type = types.str;
        default = "release";
        description = "Name of the active cargo profile.";
      };

      # ── Release Packages & Dev App Targets ───────────────────────────────────
      createLibPackages = lib.mkOption {
        type = types.bool;
        default = false;
        description = "Whether to expose workspace library packages as <crate>:lib outputs.";
      };
      libPackages = lib.mkOption {
        type = types.nullOr types.bool;
        default = null;
        description = "Alias for createLibPackages.";
      };
      createDevPackages = lib.mkOption {
        type = types.bool;
        default = false;
        description = "Whether to generate dev profile apps (<bin>:dev) for workspace binaries.";
      };
      devPackages = lib.mkOption {
        type = types.nullOr types.bool;
        default = null;
        description = "Alias for createDevPackages.";
      };
      packages = {
        createLib = lib.mkOption {
          type = types.nullOr types.bool;
          default = null;
          description = "Whether to expose workspace library packages as <crate>:lib outputs.";
        };
        createDev = lib.mkOption {
          type = types.nullOr types.bool;
          default = null;
          description = "Whether to generate dev profile apps (<bin>:dev) for workspace binaries.";
        };
      };

      # ── Custom Compiler & Runner Flags ───────────────────────────────────────
      flags = {
        rustc = lib.mkOption {
          type = types.listOf types.str;
          default = ["-Copt-level=3"];
          description = "Default rustc flags for binary and library builds.";
        };
        test = lib.mkOption {
          type = types.listOf types.str;
          default = [];
          description = "Additional rustc flags for test runner compilation.";
        };
        clippy = lib.mkOption {
          type = types.listOf types.str;
          default = [];
          description = "Additional rustc flags when running clippy-driver.";
        };
        doc = lib.mkOption {
          type = types.listOf types.str;
          default = ["-Dwarnings"];
          description = "Additional rustdoc flags for documentation generation.";
        };
        doctest = lib.mkOption {
          type = types.listOf types.str;
          default = [];
          description = "Additional rustdoc flags for doctest execution.";
        };
      };

      # ── Crate Overrides ──────────────────────────────────────────────────────
      overrides = lib.mkOption {
        type = types.attrsOf (types.submodule overrideOptionModule);
        default = {};
        description = "Per-crate build overrides for native C dependencies, extra flags, and environment variables.";
      };

      # ── Development Shell (Direnv / nix develop) ─────────────────────────────
      devShell = {
        enable = lib.mkOption {
          type = types.bool;
          default = true;
          description = "Generate a preconfigured devShell with rustc, cargo, clippy, nix-fast-build, and direnv support.";
        };
        packages = lib.mkOption {
          type = types.listOf types.package;
          default = [];
          description = "Extra packages to add to the development shell (e.g. rust-analyzer, bacon).";
        };
        shellHook = lib.mkOption {
          type = types.lines;
          default = "";
          description = "Additional bash shell hook to execute when entering the development shell.";
        };
        bloomeryCli = lib.mkOption {
          type = types.nullOr types.package;
          default = null;
          internal = true;
          description = "Bloomery CLI package injected into the development shell by mkFlake.";
        };
      };

      # ── Checks & CI ──────────────────────────────────────────────────────────
      checks = {
        enable = lib.mkOption {
          type = types.bool;
          default = true;
          description = "Generate workspace check derivations for unit tests, clippy, docs, doctests, lock validation, and optional package builds.";
        };
        includePackageChecks = lib.mkOption {
          type = types.bool;
          default = true;
          description = "Include package binary/library build checks in the checks output.";
        };
        throwOnOutOfDate = lib.mkOption {
          type = types.bool;
          default = false;
          description = "Fail evaluation immediately if Cargo.lock or bloomery.lock is out of date.";
        };
        workspaceDependencies = lib.mkOption {
          type = types.bool;
          default = true;
          description = "Require every workspace member dependency to inherit from [workspace.dependencies] using workspace = true.";
        };
        noDefaultFeatures = lib.mkOption {
          type = types.bool;
          default = true;
          description = "Require every dependency consumed by a workspace member to disable default features.";
        };
      };

      # ── Feature Resolution ───────────────────────────────────────────────────
      features = {
        unify = lib.mkOption {
          type = types.bool;
          default = true;
          description = "Unify features across workspace crates matching Cargo's workspace behavior.";
        };
        cratesIoIndex = lib.mkOption {
          type = types.nullOr types.path;
          default = null;
          description = "Path to custom crates.io index directory (null uses defaults).";
        };
      };
    };
  };

  # Evaluates raw user options into fully resolved, type-checked config
  evalWorkspaceOptions = rawConfig: let
    eval = lib.evalModules {
      modules = [
        workspaceOptionModule
        rawConfig
      ];
    };
  in
    eval.config;
in {
  inherit workspaceOptionModule evalWorkspaceOptions overrideOptionModule testOverrideOptionModule;
}
