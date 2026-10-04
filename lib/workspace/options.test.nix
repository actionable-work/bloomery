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
}
