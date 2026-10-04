{
  pkgs,
  lib ? pkgs.lib,
}: let
  mkFlake = import ./mk-flake.nix {
    bloomeryLib = {
      pkgs,
      lib,
    }:
      import ./. {inherit pkgs lib;};
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
in {
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
    expected = ["extraOutputs" "nixpkgs" "overrides" "root" "systems"];
  };
}
