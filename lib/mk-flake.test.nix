{
  pkgs,
  lib ? pkgs.lib,
  treefmtNix ? null,
  flakeParts ? null,
}: let
  bloomeryApi = {inherit mkFlake;};
  mkFlake = import ./mk-flake.nix {
    bloomeryLib = {
      pkgs,
      lib,
    }:
      import ./. {inherit pkgs lib treefmtNix;};
    defaultInputs = {bloomery = bloomeryApi;};
  };
  mkFlakeWithCli = import ./mk-flake.nix {
    bloomeryLib = {
      pkgs,
      lib,
    }:
      import ./. {inherit pkgs lib treefmtNix;};
    bloomeryCli = {pkgs, ...}: pkgs.hello;
    defaultInputs = {bloomery = bloomeryApi;};
  };

  # Main-flake `self` for tests that evaluate the repository root, whose config
  # composes the test workspaces as configured sub-flakes.
  compositionSelf = {
    inputs =
      {
        bloomery = {inherit mkFlake;};
      }
      // lib.optionalAttrs (flakeParts != null) {flake-parts = flakeParts;};
  };
  mockNixpkgs = {
    inherit lib;
    legacyPackages = {
      ${pkgs.system} = pkgs;
    };
  };
  mockNixpkgsTwoSystems = {
    inherit lib;
    legacyPackages = {
      aarch64-linux = pkgs;
      x86_64-linux = pkgs;
    };
  };

  formatterTests = lib.optionalAttrs (treefmtNix != null) {
    testMkFlakeExposesFormatter = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../tests/basic-workspace;
        };
      in
        builtins.hasAttr pkgs.system flakeOutputs.formatter
        && builtins.isAttrs flakeOutputs.formatter.${pkgs.system};
      expected = true;
    };

    testMkFlakeAcceptsExtraFormatters = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../tests/basic-workspace;
          extraFormatters.prettier = {
            package = pkgs.hello;
            includes = ["*.ts"];
          };
        };
      in
        builtins.isAttrs flakeOutputs.formatter.${pkgs.system};
      expected = true;
    };
  };
in
  {
    testMkFlakeBasicWorkspace = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../tests/basic-workspace;
        };
      in {
        hasBinCalcPackage = builtins.hasAttr "bin-calc" flakeOutputs.packages.${pkgs.system};
        hasLibCalcLibPackage = builtins.hasAttr "lib-calc:lib" flakeOutputs.packages.${pkgs.system};
        hasBinCalcDevApp = builtins.hasAttr "bin-calc:dev" flakeOutputs.apps.${pkgs.system};
        hasDefaultApp = builtins.hasAttr "default" flakeOutputs.apps.${pkgs.system};
        hasDefaultPackage = builtins.hasAttr "default" flakeOutputs.packages.${pkgs.system};
        hasDevShell = builtins.hasAttr "default" flakeOutputs.devShells.${pkgs.system};
        hasChecks = builtins.hasAttr pkgs.system flakeOutputs.checks;
        hasBloomeryCheck = builtins.hasAttr "bloomery:check" flakeOutputs.checks.${pkgs.system};
      };
      expected = {
        hasBinCalcPackage = true;
        hasLibCalcLibPackage = true;
        hasBinCalcDevApp = true;
        hasDefaultApp = true;
        hasDefaultPackage = true;
        hasDevShell = true;
        hasChecks = true;
        hasBloomeryCheck = false;
      };
    };

    testMkFlakeZeroVisibilityDefaults = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../tests/single-crate-workspace;
        };
        packages = flakeOutputs.packages.${pkgs.system};
        apps = flakeOutputs.apps.${pkgs.system};
      in {
        hasMainPackage = builtins.hasAttr "single-crate-app" packages;
        hasLibPackage = builtins.hasAttr "single-crate-app:lib" packages;
        hasDevApp = builtins.hasAttr "single-crate-app:dev" apps;
        hasDevShell = builtins.hasAttr "default" flakeOutputs.devShells.${pkgs.system};
      };
      expected = {
        hasMainPackage = true;
        hasLibPackage = false;
        hasDevApp = false;
        hasDevShell = true;
      };
    };

    testMkFlakeExplicitMainBinary = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../.;
          self = compositionSelf;
        };
        packages = flakeOutputs.packages.${pkgs.system};
      in {
        hasExplicitBinary = builtins.hasAttr "bloomery" packages;
        hasDuplicateImplicitBinary = builtins.hasAttr "bloomery-cli" packages;
        hasBloomeryCheck = builtins.hasAttr "bloomery:check" flakeOutputs.checks.${pkgs.system};
      };
      expected = {
        hasExplicitBinary = true;
        hasDuplicateImplicitBinary = false;
        hasBloomeryCheck = false;
      };
    };

    testMkFlakeExtraOutputs = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../tests/basic-workspace;
          extraOutputs = {eachSystem, ...}: {
            customAttr = eachSystem (system: "hello-${system}");
          };
        };
      in
        flakeOutputs.customAttr.${pkgs.system};
      expected = "hello-${pkgs.system}";
    };

    testMkFlakeDefaultsSystems = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          root = ../tests/basic-workspace;
        };
      in
        builtins.attrNames flakeOutputs.packages;
      expected = ["aarch64-darwin" "aarch64-linux" "x86_64-linux"];
    };

    testMkFlakeEvaluatesSelectedSystems = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgsTwoSystems;
          systems = ["x86_64-linux" "aarch64-linux"];
          root = ../tests/basic-workspace;
        };
        packages = flakeOutputs.packages;
      in {
        hasBothSystems = builtins.attrNames packages == ["aarch64-linux" "x86_64-linux"];
        evaluatesEachSystem =
          builtins.all (system: builtins.isString packages.${system}.default.drvPath) ["aarch64-linux" "x86_64-linux"];
      };
      expected = {
        hasBothSystems = true;
        evaluatesEachSystem = true;
      };
    };

    testMkFlakeDisabledDevShell = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../tests/default-bin;
        };
      in {
        hasDefaultShell = builtins.hasAttr "default" flakeOutputs.devShells.${pkgs.system};
        keepsPackages = builtins.hasAttr "default" flakeOutputs.packages.${pkgs.system};
      };
      expected = {
        hasDefaultShell = false;
        keepsPackages = true;
      };
    };

    testMkFlakeInjectsCliIntoDevShell = {
      expr = let
        flakeOutputs = mkFlakeWithCli {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../tests/basic-workspace;
        };
      in
        lib.elem pkgs.hello flakeOutputs.devShells.${pkgs.system}.default.nativeBuildInputs;
      expected = true;
    };

    testMkFlakeOmitsCliWithoutBuilder = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../tests/basic-workspace;
        };
      in
        lib.any (p: (p.pname or "") == "bloomery")
        flakeOutputs.devShells.${pkgs.system}.default.nativeBuildInputs;
      expected = false;
    };

    testMkFlakeExtraOutputsChecksAreAdditive = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../tests/composition-fixture;
          extraOutputs = {eachSystem, ...}: {
            checks = eachSystem (_system: {extra = pkgs.hello;});
          };
        };
      in {
        keepsSubFlakeCheck = builtins.hasAttr "complete:complete:check" flakeOutputs.checks.${pkgs.system};
        addsExtraCheck = builtins.hasAttr "extra" flakeOutputs.checks.${pkgs.system};
      };
      expected = {
        keepsSubFlakeCheck = true;
        addsExtraCheck = true;
      };
    };

    testMkFlakeExtraOutputsAppsMerge = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../tests/basic-workspace;
          extraOutputs = {
            eachSystem,
            perSystemWorkspace,
            ...
          }: {
            apps = eachSystem (
              system:
                perSystemWorkspace.${system}.apps
                // {
                  bench = {
                    type = "app";
                    program = "${pkgs.hello}/bin/hello";
                  };
                }
            );
          };
        };
        apps = flakeOutputs.apps.${pkgs.system};
      in {
        keepsWorkspaceApp = builtins.hasAttr "bin-calc:dev" apps;
        addsBenchApp = apps.bench.type == "app";
      };
      expected = {
        keepsWorkspaceApp = true;
        addsBenchApp = true;
      };
    };

    testMkFlakeExtraOutputsWorkspaceContext = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../tests/basic-workspace;
          extraOutputs = {
            eachSystem,
            perSystemWorkspace,
          }: {
            eachSystemProbe = eachSystem (system: "probe-${system}");
            workspaceCrateIds = builtins.attrNames perSystemWorkspace.${pkgs.system}.crates;
            workspaceLockVersion = perSystemWorkspace.${pkgs.system}.lock.version;
          };
        };
      in {
        probe = flakeOutputs.eachSystemProbe.${pkgs.system};
        hasLocalCrate = builtins.elem "bin-calc-0.1.0" flakeOutputs.workspaceCrateIds;
        lockVersion = flakeOutputs.workspaceLockVersion;
      };
      expected = {
        probe = "probe-${pkgs.system}";
        hasLocalCrate = true;
        lockVersion = 4;
      };
    };

    testMkFlakeExtraOutputsOverride = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../tests/basic-workspace;
          extraOutputs = _: {
            packages = {"override-marker" = "kept";};
          };
        };
      in
        flakeOutputs.packages;
      expected = {"override-marker" = "kept";};
    };

    testMkFlakeOverridesArgument = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../tests/basic-workspace;
          overrides.bin-calc.env.FLAKE_OVERRIDE = "yes";
        };
      in
        flakeOutputs.packages.${pkgs.system}."bin-calc".FLAKE_OVERRIDE;
      expected = "yes";
    };

    testMkFlakeRequiresConfiguration = {
      expr =
        !(builtins.tryEval (
          builtins.deepSeq (mkFlake {
            nixpkgs = mockNixpkgs;
            systems = [pkgs.system];
            root = ../nix;
          })
          true
        )).success;
      expected = true;
    };

    testMkFlakeArgumentSurface = {
      expr = builtins.attrNames (builtins.functionArgs mkFlake);
      expected = ["extraFormatters" "extraOutputs" "nixpkgs" "overrides" "root" "self" "systems"];
    };

    testMkFlakeCompositionOffByDefault = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../tests/default-bin;
        };
      in {
        packages = builtins.attrNames flakeOutputs.packages.${pkgs.system};
        apps = builtins.attrNames flakeOutputs.apps.${pkgs.system};
      };
      expected = {
        packages = ["default" "host-app"];
        apps = ["default" "host-app" "host-app:doc"];
      };
    };

    testMkFlakeElevatesPackagesAndApps = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../tests/composition-fixture;
        };
        packages = flakeOutputs.packages.${pkgs.system};
        apps = flakeOutputs.apps.${pkgs.system};
      in {
        hasMainPackage = builtins.hasAttr "default" packages;
        hasMainApp = builtins.hasAttr "default" apps;
        hasSubPackage = builtins.hasAttr "complete:complete-pkg" packages;
        hasOnlyPackage = builtins.hasAttr "packages-only:only-pkg" packages;
        hasSubApp = builtins.hasAttr "complete:complete-app" apps;
        subAppIsApp = apps."complete:complete-app".type == "app";
        offContributesNothing = !(lib.any (name: lib.hasPrefix "off:" name) (builtins.attrNames packages ++ builtins.attrNames apps));
      };
      expected = {
        hasMainPackage = true;
        hasMainApp = true;
        hasSubPackage = true;
        hasOnlyPackage = true;
        hasSubApp = true;
        subAppIsApp = true;
        offContributesNothing = true;
      };
    };

    testMkFlakeElevatesIndividualChecksAndAggregate = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../tests/composition-fixture;
          self = compositionSelf;
        };
        checks = flakeOutputs.checks.${pkgs.system};
        aggregate = checks."aggregate:checks";
      in {
        hasCheck = builtins.hasAttr "complete:complete:check" checks;
        hasOther = builtins.hasAttr "complete:complete:other" checks;
        hasAggregate = builtins.hasAttr "aggregate:checks" checks;
        aggregateIsDerivation = aggregate.type == "derivation";
        mainChecksDisabled = !(builtins.hasAttr "workspace:lock" checks);
      };
      expected = {
        hasCheck = true;
        hasOther = true;
        hasAggregate = true;
        aggregateIsDerivation = true;
        mainChecksDisabled = true;
      };
    };

    testMkFlakeSkipsMissingSubFlakeFamilies = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../tests/composition-fixture;
        };
        apps = flakeOutputs.apps.${pkgs.system};
        checks = flakeOutputs.checks.${pkgs.system};
      in {
        noMissingApps = !(lib.any (name: lib.hasPrefix "packages-only:" name) (builtins.attrNames apps));
        noMissingChecks = !(lib.any (name: lib.hasPrefix "packages-only:" name) (builtins.attrNames checks));
      };
      expected = {
        noMissingApps = true;
        noMissingChecks = true;
      };
    };

    testMkFlakeCompositionEvaluatesPurely = {
      expr =
        (builtins.tryEval (
          builtins.attrNames
          (mkFlake {
            nixpkgs = mockNixpkgs;
            systems = [pkgs.system];
            root = ../tests/composition-fixture;
          }).checks.${
            pkgs.system
          }
        )).success;
      expected = true;
    };

    testMkFlakeElevatedCollisionFails = {
      expr =
        !(builtins.tryEval (
          builtins.attrNames
          (mkFlake {
            nixpkgs = mockNixpkgs;
            systems = [pkgs.system];
            root = ../tests/composition-collision-fixture;
          }).checks.${
            pkgs.system
          }
        )).success;
      expected = true;
    };

    testMkFlakeRepositoryComposition = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../.;
          self = compositionSelf;
        };
        checks = flakeOutputs.checks.${pkgs.system};
        names = builtins.attrNames checks;
      in {
        hasBasicCheck = lib.any (name: lib.hasPrefix "basic-workspace:" name) names;
        hasAxumCheck = lib.any (name: lib.hasPrefix "axum-workspace:" name) names;
      };
      expected = {
        hasBasicCheck = true;
        hasAxumCheck = true;
      };
    };

    testMkFlakeNestedComposition = {
      expr = let
        flakeOutputs = mkFlake {
          nixpkgs = mockNixpkgs;
          systems = [pkgs.system];
          root = ../tests/nested-composition-fixture;
        };
        checks = flakeOutputs.checks.${pkgs.system};
        names = builtins.attrNames checks;
      in {
        hasLevelOne = lib.any (name: lib.hasPrefix "level-one:" name) names;
        hasNestedLevelTwo = builtins.hasAttr "level-one:level-two:check" checks;
      };
      expected = {
        hasLevelOne = true;
        hasNestedLevelTwo = true;
      };
    };
  }
  // formatterTests
