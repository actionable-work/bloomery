{
  pkgs,
  lib ? pkgs.lib,
}: let
  options = import ./options.nix {inherit pkgs lib;};
in {
  testDefaultOptions = {
    expr = let
      cfg = options.evalWorkspaceOptions {
        root = ./.;
      };
    in {
      hasRoot = cfg.root == ./.;
      hasCargoToml = cfg.source.cargoToml == ./. + "/Cargo.toml";
      hasCargoLock = cfg.source.cargoLock == ./. + "/Cargo.lock";
      devShellEnabled = cfg.devShell.enable;
      checksEnabled = cfg.checks.enable;
      unifyFeatures = cfg.features.unify;
      linker = cfg.toolchain.linker;
      libPackages = cfg.createLibPackages;
      devPackages = cfg.createDevPackages;
      workspaceDependencies = cfg.checks.workspaceDependencies;
      noDefaultFeatures = cfg.checks.noDefaultFeatures;
    };
    expected = {
      hasRoot = true;
      hasCargoToml = true;
      hasCargoLock = true;
      devShellEnabled = true;
      checksEnabled = true;
      unifyFeatures = true;
      linker =
        if pkgs.stdenv.hostPlatform.isLinux
        then "lld"
        else null;
      libPackages = false;
      devPackages = false;
      workspaceDependencies = true;
      noDefaultFeatures = true;
    };
  };

  testManifestCheckOptions = {
    expr = let
      cfg = options.evalWorkspaceOptions {
        root = ./.;
        checks.workspaceDependencies = false;
        checks.noDefaultFeatures = false;
      };
    in {
      workspaceDependencies = cfg.checks.workspaceDependencies;
      noDefaultFeatures = cfg.checks.noDefaultFeatures;
    };
    expected = {
      workspaceDependencies = false;
      noDefaultFeatures = false;
    };
  };

  testCategorizedOverrides = {
    expr = let
      cfg = options.evalWorkspaceOptions {
        root = ./.;
        toolchain.linker = "mold";
        flags.test = ["--nocapture"];
        profile = {
          optLevel = 2;
          lto = "thin";
          strip = true;
        };
        devShell.packages = [pkgs.hello];
        checks.throwOnOutOfDate = true;
      };
    in {
      linker = cfg.toolchain.linker;
      testFlags = cfg.flags.test;
      profileOptLevel = cfg.profile.optLevel;
      profileLto = cfg.profile.lto;
      profileStrip = cfg.profile.strip;
      devShellPackagesCount = builtins.length cfg.devShell.packages;
      throwOnOutOfDate = cfg.checks.throwOnOutOfDate;
    };
    expected = {
      linker = "mold";
      testFlags = ["--nocapture"];
      profileOptLevel = 2;
      profileLto = "thin";
      profileStrip = true;
      devShellPackagesCount = 1;
      throwOnOutOfDate = true;
    };
  };

  testOverrideOptionSubmodule = {
    expr = let
      cfg = options.evalWorkspaceOptions {
        root = ./.;
        overrides = {
          foo-sys = {
            nativeBuildInputs = [pkgs.hello];
            rustcFlags = ["-Cprefer-dynamic"];
            env = {FOO = "bar";};
            profile = {optLevel = 1;};
            src = ./.;
          };
        };
      };
    in {
      fooNativeDeps = builtins.length cfg.overrides.foo-sys.nativeBuildInputs;
      fooFlags = cfg.overrides.foo-sys.rustcFlags;
      fooEnv = cfg.overrides.foo-sys.env.FOO;
      fooProfileOptLevel = cfg.overrides.foo-sys.profile.optLevel;
      hasSrc = cfg.overrides.foo-sys.src == ./.;
    };
    expected = {
      fooNativeDeps = 1;
      fooFlags = ["-Cprefer-dynamic"];
      fooEnv = "bar";
      fooProfileOptLevel = 1;
      hasSrc = true;
    };
  };

  testTestOverrideSubmoduleIsSeparateFromCommonInputs = {
    expr = let
      cfg = options.evalWorkspaceOptions {
        root = ./.;
        overrides = {
          foo-sys = {
            nativeBuildInputs = [pkgs.hello];
            test = {
              buildInputs = [pkgs.zlib];
              env = {FOO_TEST = "1";};
              fileset = ./options.test.nix;
            };
          };
        };
      };
    in {
      commonNativeDeps = builtins.length cfg.overrides.foo-sys.nativeBuildInputs;
      testBuildDeps = builtins.length cfg.overrides.foo-sys.test.buildInputs;
      testEnv = cfg.overrides.foo-sys.test.env.FOO_TEST;
      hasTestFileset = cfg.overrides.foo-sys.test.fileset != null;
      testNativeDepsDefault = cfg.overrides.foo-sys.test.nativeBuildInputs;
    };
    expected = {
      commonNativeDeps = 1;
      testBuildDeps = 1;
      testEnv = "1";
      hasTestFileset = true;
      testNativeDepsDefault = [];
    };
  };

  testToolchainOptionDefaults = {
    expr = let
      cfg = options.evalWorkspaceOptions {
        root = ./.;
      };
    in {
      rustc = cfg.toolchain.rustc == pkgs.rustc;
      clippy = cfg.toolchain.clippy == pkgs.clippy;
      cargo = cfg.toolchain.cargo == pkgs.cargo;
      lld = cfg.toolchain.lld == pkgs.lld;
      mold = cfg.toolchain.mold == pkgs.mold;
      stdenv = cfg.toolchain.stdenv == pkgs.stdenv;
    };
    expected = {
      rustc = true;
      clippy = true;
      cargo = true;
      lld = true;
      mold = true;
      stdenv = true;
    };
  };

  testToolchainOptionOverrides = {
    expr = let
      cfg = options.evalWorkspaceOptions {
        root = ./.;
        toolchain = {
          rustc = pkgs.rustc;
          clippy = pkgs.clippy;
          cargo = pkgs.cargo;
          linker = "mold";
          lld = pkgs.lld;
          mold = pkgs.mold;
          stdenv = pkgs.stdenv;
        };
      };
    in {
      linker = cfg.toolchain.linker;
      rustc = cfg.toolchain.rustc == pkgs.rustc;
    };
    expected = {
      linker = "mold";
      rustc = true;
    };
  };

  testProfileOptionFields = {
    expr = let
      cfg = options.evalWorkspaceOptions {
        root = ./.;
        profile = {
          optLevel = "s";
          lto = "fat";
          codegenUnits = 4;
          panic = "abort";
          strip = "debuginfo";
          targetCpu = "x86-64-v3";
          debuginfo = 1;
          overflowChecks = false;
          linker = "cc";
          linkArgs = ["-Wl,-z,now"];
        };
      };
    in {
      inherit (cfg.profile) optLevel lto codegenUnits panic strip targetCpu debuginfo overflowChecks linker linkArgs;
    };
    expected = {
      optLevel = "s";
      lto = "fat";
      codegenUnits = 4;
      panic = "abort";
      strip = "debuginfo";
      targetCpu = "x86-64-v3";
      debuginfo = 1;
      overflowChecks = false;
      linker = "cc";
      linkArgs = ["-Wl,-z,now"];
    };
  };

  testVisibilityAliasesExposed = {
    expr = let
      cfg = options.evalWorkspaceOptions {
        root = ./.;
        createLibPackages = false;
        createDevPackages = false;
        libPackages = true;
        packages.createLib = true;
        devPackages = true;
        packages.createDev = true;
      };
    in {
      libAlias = cfg.libPackages;
      createLib = cfg.packages.createLib;
      devAlias = cfg.devPackages;
      createDev = cfg.packages.createDev;
      libToggle = cfg.createLibPackages;
      devToggle = cfg.createDevPackages;
    };
    expected = {
      libAlias = true;
      createLib = true;
      devAlias = true;
      createDev = true;
      libToggle = false;
      devToggle = false;
    };
  };

  testAllFlagLists = {
    expr = let
      cfg = options.evalWorkspaceOptions {
        root = ./.;
        flags = {
          rustc = ["--cfg=a"];
          test = ["--cfg=b"];
          clippy = ["--cfg=c"];
          doc = ["--cfg=d"];
          doctest = ["--cfg=e"];
        };
      };
    in {
      inherit (cfg.flags) rustc test clippy doc doctest;
    };
    expected = {
      rustc = ["--cfg=a"];
      test = ["--cfg=b"];
      clippy = ["--cfg=c"];
      doc = ["--cfg=d"];
      doctest = ["--cfg=e"];
    };
  };

  testOverrideFields = {
    expr = let
      cfg = options.evalWorkspaceOptions {
        root = ./.;
        overrides.foo-sys = {
          buildInputs = [pkgs.zlib];
          rustdocFlags = ["--html-in-header=header.html"];
          features = ["alpha" "beta"];
          fileset = ./options.test.nix;
          profileDev = {optLevel = 1;};
          assets = [./options.test.nix];
          assetDirs = ["templates" "styles"];
        };
      };
      ovr = cfg.overrides.foo-sys;
    in {
      buildInputs = builtins.length ovr.buildInputs;
      rustdocFlags = ovr.rustdocFlags;
      features = ovr.features;
      hasFileset = ovr.fileset != null;
      profileDevOpt = ovr.profileDev.optLevel;
      assets = builtins.length ovr.assets;
      assetDirs = ovr.assetDirs;
    };
    expected = {
      buildInputs = 1;
      rustdocFlags = ["--html-in-header=header.html"];
      features = ["alpha" "beta"];
      hasFileset = true;
      profileDevOpt = 1;
      assets = 1;
      assetDirs = ["templates" "styles"];
    };
  };

  testDevShellShellHook = {
    expr = let
      cfg = options.evalWorkspaceOptions {
        root = ./.;
        devShell.shellHook = "echo hello";
      };
    in
      cfg.devShell.shellHook;
    expected = "echo hello";
  };

  testCratesIoIndexOption = {
    expr = let
      cfg = options.evalWorkspaceOptions {
        root = ./.;
        features.cratesIoIndex = ../tests/crates-index;
      };
    in
      cfg.features.cratesIoIndex == ../tests/crates-index;
    expected = true;
  };
}
