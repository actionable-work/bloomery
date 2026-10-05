{
  pkgs,
  lib ? pkgs.lib,
}: let
  loader = import ./build-config.nix {inherit pkgs lib;};
  options = import ./workspace/options.nix {inherit pkgs lib;};

  eval = config:
    options.evalWorkspaceOptions (loader.toWorkspaceArgs {
      root = ./.;
      inherit config;
    });

  fails = value: !(builtins.tryEval (builtins.deepSeq value true)).success;
in {
  testBuildConfigDefaults = {
    expr = let
      cfg = eval {};
    in {
      cargoToml = cfg.source.cargoToml == ./. + "/Cargo.toml";
      cargoLock = cfg.source.cargoLock == ./. + "/Cargo.lock";
      libPackages = cfg.createLibPackages;
      devPackages = cfg.createDevPackages;
      devShellEnabled = cfg.devShell.enable;
      checksEnabled = cfg.checks.enable;
      unify = cfg.build.unify;
      profileOptLevel = cfg.profile.optLevel;
    };
    expected = {
      cargoToml = true;
      cargoLock = true;
      libPackages = false;
      devPackages = false;
      devShellEnabled = true;
      checksEnabled = true;
      unify = true;
      profileOptLevel = null;
    };
  };

  testBuildTableIsMapped = {
    expr = let
      cfg = eval {
        build = {
          cargoToml = "Cargo.toml";
          cargoLock = "Cargo.lock";
          bloomeryLock = "bloomery.lock";
          members = ["alpha"];
          profileName = "dev";
          libPackages = true;
          devPackages = true;
        };
      };
    in {
      cargoToml = cfg.source.cargoToml;
      cargoLock = cfg.source.cargoLock;
      bloomeryLock = cfg.source.bloomeryLock;
      members = cfg.source.members;
      profileName = cfg.profileName;
      libPackages = cfg.createLibPackages;
      devPackages = cfg.createDevPackages;
    };
    expected = {
      cargoToml = ./. + "/Cargo.toml";
      cargoLock = ./. + "/Cargo.lock";
      bloomeryLock = ./. + "/bloomery.lock";
      members = ["alpha"];
      profileName = "dev";
      libPackages = true;
      devPackages = true;
    };
  };

  testToolchainPackagesResolve = {
    expr = let
      cfg = eval {
        toolchain = {
          rustc = "rustc";
          clippy = "clippy";
          cargo = "cargo";
          lld = "lld";
          mold = "mold";
          stdenv = "stdenv";
          linker = "mold";
        };
      };
    in {
      rustc = cfg.toolchain.rustc == pkgs.rustc;
      clippy = cfg.toolchain.clippy == pkgs.clippy;
      cargo = cfg.toolchain.cargo == pkgs.cargo;
      lld = cfg.toolchain.lld == pkgs.lld;
      mold = cfg.toolchain.mold == pkgs.mold;
      stdenv = cfg.toolchain.stdenv == pkgs.stdenv;
      linker = cfg.toolchain.linker;
    };
    expected = {
      rustc = true;
      clippy = true;
      cargo = true;
      lld = true;
      mold = true;
      stdenv = true;
      linker = "mold";
    };
  };

  testProfilesAndFlagsAreMapped = {
    expr = let
      cfg = eval {
        profile = {
          release = {
            optLevel = 2;
            lto = "thin";
          };
          dev.optLevel = 1;
        };
        flags = {
          rustc = ["-Copt-level=2"];
          doc = ["-Dwarnings"];
        };
      };
    in {
      profileOptLevel = cfg.profile.optLevel;
      profileLto = cfg.profile.lto;
      profileDevOptLevel = cfg.profileDev.optLevel;
      rustcFlags = cfg.flags.rustc;
      docFlags = cfg.flags.doc;
    };
    expected = {
      profileOptLevel = 2;
      profileLto = "thin";
      profileDevOptLevel = 1;
      rustcFlags = ["-Copt-level=2"];
      docFlags = ["-Dwarnings"];
    };
  };

  testDevShellChecksAndFeaturesAreMapped = {
    expr = let
      cfg = eval {
        devShell = {
          enable = false;
          packages = ["hello"];
          shellHook = "echo hello";
        };
        checks = {
          enable = false;
          includePackageChecks = false;
          throwOnOutOfDate = true;
          workspaceDependencies = false;
          noDefaultFeatures = false;
        };
        build = {
          unify = false;
        };
        features = {
          cratesIoIndex = "crates-io-index";
        };
      };
    in {
      devShellEnabled = cfg.devShell.enable;
      devShellPackages = cfg.devShell.packages == [pkgs.hello];
      shellHook = cfg.devShell.shellHook;
      checksEnabled = cfg.checks.enable;
      includePackageChecks = cfg.checks.includePackageChecks;
      throwOnOutOfDate = cfg.checks.throwOnOutOfDate;
      workspaceDependencies = cfg.checks.workspaceDependencies;
      noDefaultFeatures = cfg.checks.noDefaultFeatures;
      unify = cfg.build.unify;
      cratesIoIndex = cfg.features.cratesIoIndex;
    };
    expected = {
      devShellEnabled = false;
      devShellPackages = true;
      shellHook = "echo hello";
      checksEnabled = false;
      includePackageChecks = false;
      throwOnOutOfDate = true;
      workspaceDependencies = false;
      noDefaultFeatures = false;
      unify = false;
      cratesIoIndex = ./. + "/crates-io-index";
    };
  };

  testNonBuildTablesAreIgnored = {
    expr = let
      cfg = eval {
        specs.dir = "specs";
        scanners.rust.enabled = true;
        build.libPackages = true;
      };
    in {
      libPackages = cfg.createLibPackages;
      cargoToml = cfg.source.cargoToml == ./. + "/Cargo.toml";
    };
    expected = {
      libPackages = true;
      cargoToml = true;
    };
  };

  testUnknownBuildKeyFails = {
    expr = fails (loader.toWorkspaceArgs {
      root = ./.;
      config.build.bogus = true;
    });
    expected = true;
  };

  testWrongTableTypeFails = {
    expr = fails (loader.toWorkspaceArgs {
      root = ./.;
      config.build = "not-a-table";
    });
    expected = true;
  };

  testWrongPathTypeFails = {
    expr = fails (loader.toWorkspaceArgs {
      root = ./.;
      config.build.cargoToml = 1;
    });
    expected = true;
  };

  testUnresolvablePackageFails = {
    expr = fails (loader.toWorkspaceArgs {
      root = ./.;
      config.toolchain.rustc = "definitely-not-a-real-package";
    });
    expected = true;
  };

  testMissingConfigurationFileFails = {
    expr = fails (loader.readConfig ./nonexistent-workspace-root);
    expected = true;
  };

  testValidConfigurationFileIsRead = {
    expr = let
      config = loader.readConfig ../tests/config-fixtures/valid;
      cfg = options.evalWorkspaceOptions (loader.toWorkspaceArgs {
        root = ../tests/config-fixtures/valid;
        inherit config;
      });
    in {
      libPackages = cfg.createLibPackages;
      devPackages = cfg.createDevPackages;
      linker = cfg.toolchain.linker;
      profileOptLevel = cfg.profile.optLevel;
    };
    expected = {
      libPackages = true;
      devPackages = true;
      linker = "mold";
      profileOptLevel = 2;
    };
  };

  testFormattersTableIsMapped = {
    expr = let
      cfg = eval {
        formatters.toml-sort.enable = false;
        formatters.rustfmt.before = ["alejandra"];
      };
    in {
      tomlSortDisabled = cfg.formatters.toml-sort.enable == false;
      rustfmtBefore = cfg.formatters.rustfmt.before;
    };
    expected = {
      tomlSortDisabled = true;
      rustfmtBefore = ["alejandra"];
    };
  };

  testUnknownFormatterKeyFails = {
    expr = fails (loader.toWorkspaceArgs {
      root = ./.;
      config.formatters.prettier.command = "prettier";
    });
    expected = true;
  };
}
