{
  pkgs,
  lib ? pkgs.lib,
}: let
  rawLoader = import ./build-config.nix {inherit pkgs lib;};
  loader =
    rawLoader
    // {
      toWorkspaceArgs = request: let
        config = request.config or {};
      in
        rawLoader.toWorkspaceArgs (request // {config = {version = 1;} // config;});
    };
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
      includePackageChecks = cfg.checks.includePackageChecks;
      crateCheck = cfg.checks.crateCheck;
      workspaceDependencies = cfg.checks.workspaceDependencies;
      noDefaultFeatures = cfg.checks.noDefaultFeatures;
      unify = cfg.build.unify;
      profileOptLevel = cfg.profile.optLevel;
    };
    expected = {
      cargoToml = true;
      cargoLock = true;
      libPackages = false;
      devPackages = false;
      devShellEnabled = true;
      checksEnabled = false;
      includePackageChecks = false;
      crateCheck = false;
      workspaceDependencies = false;
      noDefaultFeatures = false;
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
          crateCheck = false;
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
      crateCheck = cfg.checks.crateCheck;
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
      crateCheck = false;
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

  testCodeHealthConfigIsValidatedAndNotPassedToNixChecks = {
    expr = let
      args = loader.toWorkspaceArgs {
        root = ./.;
        config.checks.codeHealth = {
          enable = false;
          duplicationFreeRatio = 0.25;
        };
      };
    in {
      cliPolicyRemoved = !(args.checks ? codeHealth);
      invalidRatio = fails (loader.toWorkspaceArgs {
        root = ./.;
        config.checks.codeHealth.duplicationFreeRatio = 1.0;
      });
      hardBelowSoft = fails (loader.toWorkspaceArgs {
        root = ./.;
        config.checks.codeHealth = {
          cyclomaticLimit = 10;
          maxCyclomatic = 9;
        };
      });
      badWindow = fails (loader.toWorkspaceArgs {
        root = ./.;
        config.checks.codeHealth.duplicateMinTokens = 15;
      });
    };
    expected = {
      cliPolicyRemoved = true;
      invalidRatio = true;
      hardBelowSoft = true;
      badWindow = true;
    };
  };

  testCliCheckPolicyTablesAreValidatedAndNotPassedToNix = {
    expr = let
      args = loader.toWorkspaceArgs {
        root = ./.;
        config.checks.specifications.enable = true;
        config.checks.clippySuppressions = {
          enable = true;
          allow = ["clippy::unwrap_used"];
        };
      };
    in {
      policiesRemoved = !(args.checks ? specifications) && !(args.checks ? clippySuppressions);
      badSpecType = fails (loader.toWorkspaceArgs {
        root = ./.;
        config.checks.specifications.enable = "true";
      });
      badLintName = fails (loader.toWorkspaceArgs {
        root = ./.;
        config.checks.clippySuppressions.allow = ["unwrap_used"];
      });
      duplicateLint = fails (loader.toWorkspaceArgs {
        root = ./.;
        config.checks.clippySuppressions.allow = ["clippy::unwrap_used" "clippy::unwrap_used"];
      });
    };
    expected = {
      policiesRemoved = true;
      badSpecType = true;
      badLintName = true;
      duplicateLint = true;
    };
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

  testUnversionedConfigurationFileFails = {
    expr = fails (loader.readConfig ../tests/config-fixtures/unversioned);
    expected = true;
  };

  testConfigVersionValidation = {
    expr = {
      missing = fails (rawLoader.toWorkspaceArgs {
        root = ./.;
        config = {};
      });
      zero = fails (rawLoader.toWorkspaceArgs {
        root = ./.;
        config.version = 0;
      });
      negative = fails (rawLoader.toWorkspaceArgs {
        root = ./.;
        config.version = -1;
      });
      float = fails (rawLoader.toWorkspaceArgs {
        root = ./.;
        config.version = 1.0;
      });
      string = fails (rawLoader.toWorkspaceArgs {
        root = ./.;
        config.version = "1";
      });
      boolean = fails (rawLoader.toWorkspaceArgs {
        root = ./.;
        config.version = true;
      });
      future = fails (rawLoader.toWorkspaceArgs {
        root = ./.;
        config.version = 2;
      });
      current =
        !fails (rawLoader.toWorkspaceArgs {
          root = ./.;
          config.version = 1;
        });
    };
    expected = {
      missing = true;
      zero = true;
      negative = true;
      float = true;
      string = true;
      boolean = true;
      future = true;
      current = true;
    };
  };

  testValidConfigurationFileIsRead = {
    expr = let
      config = loader.readConfig ../tests/config-fixtures/valid;
      cfg = options.evalWorkspaceOptions (loader.toWorkspaceArgs {
        root = ../tests/config-fixtures/valid;
        inherit config;
      });
    in {
      version = config.version;
      libPackages = cfg.createLibPackages;
      devPackages = cfg.createDevPackages;
      linker = cfg.toolchain.linker;
      profileOptLevel = cfg.profile.optLevel;
    };
    expected = {
      version = 1;
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

  testOptimizeTableIsMapped = {
    expr = let
      config = loader.readConfig ../tests/config-fixtures/optimize;
      cfg = options.evalWorkspaceOptions (loader.toWorkspaceArgs {
        root = ../tests/config-fixtures/optimize;
        inherit config;
      });
    in {
      enabled = cfg.optimize.my-server.enable;
      script = cfg.optimize.my-server.script == ../tests/config-fixtures/optimize + "/scripts/train-my-server.sh";
      systemTargetCpu = cfg.optimize.my-server.systems.x86_64-linux.targetCpu;
      pgoEnable = cfg.optimize.my-server.pgo.enable;
      pgoScope = cfg.optimize.my-server.pgo.scope;
      boltEnable = cfg.optimize.my-server.bolt.enable;
      boltFunctions = cfg.optimize.my-server.bolt.functions;
      boltBlocks = cfg.optimize.my-server.bolt.blocks;
      disabledScript = cfg.optimize.disabled-server.script;
      entryCount = builtins.length (builtins.attrNames cfg.optimize);
    };
    expected = {
      enabled = true;
      script = true;
      systemTargetCpu = "x86-64-v4";
      pgoEnable = true;
      pgoScope = "all";
      boltEnable = true;
      boltFunctions = false;
      boltBlocks = true;
      disabledScript = null;
      entryCount = 2;
    };
  };

  testOptimizeInvalidShapesFail = {
    expr = map fails [
      (loader.toWorkspaceArgs {
        root = ./.;
        config.optimize = "not-a-table";
      })
      (loader.toWorkspaceArgs {
        root = ./.;
        config.optimize.my-server = "not-a-table";
      })
      (loader.toWorkspaceArgs {
        root = ./.;
        config.optimize.my-server = {
          enable = "yes";
          script = "train.sh";
        };
      })
      (loader.toWorkspaceArgs {
        root = ./.;
        config.optimize.my-server = {
          enable = true;
          script = "train.sh";
          bogus = true;
        };
      })
      (loader.toWorkspaceArgs {
        root = ./.;
        config.optimize.my-server = {
          enable = true;
          targetCpu = "";
        };
      })
      (loader.toWorkspaceArgs {
        root = ./.;
        config.optimize.my-server = {
          enable = true;
          script = "train.sh";
          systems = "x86_64-linux";
        };
      })
      (loader.toWorkspaceArgs {
        root = ./.;
        config.optimize.my-server = {
          enable = true;
          script = "train.sh";
          systems.x86_64-linux.targetCpu = 5;
        };
      })
      (loader.toWorkspaceArgs {
        root = ./.;
        config.optimize.my-server = {
          enable = true;
          script = "train.sh";
          systems.x86_64-linux.bogus = true;
        };
      })
      (loader.toWorkspaceArgs {
        root = ./.;
        config.optimize.my-server = {
          enable = true;
          script = "train.sh";
          pgo = {scope = "everything";};
        };
      })
      (loader.toWorkspaceArgs {
        root = ./.;
        config.optimize.my-server = {
          enable = true;
          script = "train.sh";
          bolt = {
            functions = false;
            blocks = false;
          };
        };
      })
    ];
    expected = [true true true true true true true true true true];
  };

  testOptimizeScriptValidation = {
    expr = let
      disabledCfg = options.evalWorkspaceOptions (loader.toWorkspaceArgs {
        root = ./.;
        config.optimize.my-server = {enable = false;};
      });
      stagesDisabledCfg = options.evalWorkspaceOptions (loader.toWorkspaceArgs {
        root = ./.;
        config.optimize.my-server = {
          enable = true;
          pgo = {enable = false;};
          bolt = {enable = false;};
        };
      });
    in {
      # Enabled entries with a training stage require a non-empty script.
      enabledWithoutScript = fails (loader.toWorkspaceArgs {
        root = ./.;
        config.optimize.my-server = {enable = true;};
      });
      emptyScript = fails (loader.toWorkspaceArgs {
        root = ./.;
        config.optimize.my-server = {
          enable = true;
          script = "";
        };
      });
      # Scripts must stay inside the workspace root.
      escapingScript = fails (loader.toWorkspaceArgs {
        root = ./.;
        config.optimize.my-server = {
          enable = true;
          script = "../escape.sh";
        };
      });
      absoluteScript = fails (loader.toWorkspaceArgs {
        root = ./.;
        config.optimize.my-server = {
          enable = true;
          script = "/absolute.sh";
        };
      });
      # A disabled entry does not require a script.
      disabledIsAccepted =
        disabledCfg.optimize.my-server.enable
        == false
        && disabledCfg.optimize.my-server.script == null;
      # An entry with every stage disabled needs no script either.
      stagesDisabledAccept =
        stagesDisabledCfg.optimize.my-server.script
        == null
        && stagesDisabledCfg.optimize.my-server.pgo.enable == false
        && stagesDisabledCfg.optimize.my-server.bolt.enable == false;
    };
    expected = {
      enabledWithoutScript = true;
      emptyScript = true;
      escapingScript = true;
      absoluteScript = true;
      disabledIsAccepted = true;
      stagesDisabledAccept = true;
    };
  };

  testFlakeElevationDefaults = {
    expr = let
      flakes = loader.checkFlakes {
        tests-basic.path = "tests/basic-workspace";
      };
    in {
      path = flakes.tests-basic.path;
      packages = flakes.tests-basic.packages;
      apps = flakes.tests-basic.apps;
      checks = flakes.tests-basic.checks;
    };
    expected = {
      path = "tests/basic-workspace";
      packages = false;
      apps = false;
      checks = "none";
    };
  };

  testFlakeInvalidShapesFail = {
    expr = map fails [
      (loader.checkFlakes {x = {packages = true;};})
      (loader.checkFlakes {
        x = {
          path = "tests/x";
          bogus = true;
        };
      })
      (loader.checkFlakes {
        x = {
          path = "tests/x";
          packages = "yes";
        };
      })
      (loader.checkFlakes {
        x = {
          path = "tests/x";
          apps = 1;
        };
      })
      (loader.checkFlakes {
        x = {
          path = "tests/x";
          checks = "always";
        };
      })
      (loader.checkFlakes {x = {path = "../outside";};})
      (loader.checkFlakes {x = {path = "/absolute";};})
    ];
    expected = [true true true true true true true];
  };

  testFlakeMissingDirectoryFails = {
    expr = fails (loader.loadFlakes ../tests/composition-missing-fixture);
    expected = true;
  };
}
