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
in {
  testMkFlakeDefaults = {
    expr = let
      flakeOutputs = mkFlake {
        nixpkgs = mockNixpkgs;
        systems = [pkgs.system];
        root = ../tests/basic-workspace;
      };
    in {
      hasPackages = builtins.hasAttr pkgs.system flakeOutputs.packages;
      hasBinCalcPackage = builtins.hasAttr "bin-calc" flakeOutputs.packages.${pkgs.system};
      hasBinCalcDevColonPackage = builtins.hasAttr "bin-calc:dev" flakeOutputs.packages.${pkgs.system};
      hasBinCalcDevDashPackage = builtins.hasAttr "bin-calc-dev" flakeOutputs.packages.${pkgs.system};
      hasLibCalcPackage = builtins.hasAttr "lib-calc" flakeOutputs.packages.${pkgs.system};
      hasLibCalcDevColonPackage = builtins.hasAttr "lib-calc:dev" flakeOutputs.packages.${pkgs.system};
      hasLibCalcLibDevColonPackage = builtins.hasAttr "lib-calc-lib:dev" flakeOutputs.packages.${pkgs.system};
      hasLibCalcCheck = builtins.hasAttr "lib-calc:lib" flakeOutputs.checks.${pkgs.system};
      hasApps = builtins.hasAttr pkgs.system flakeOutputs.apps;
      hasBinCalcDevColonApp = builtins.hasAttr "bin-calc:dev" flakeOutputs.apps.${pkgs.system};
      hasBinCalcDevDashApp = builtins.hasAttr "bin-calc-dev" flakeOutputs.apps.${pkgs.system};
      hasLockApp = builtins.hasAttr "lock" flakeOutputs.apps.${pkgs.system};
      hasDevShell = builtins.hasAttr "default" flakeOutputs.devShells.${pkgs.system};
      hasChecks = builtins.hasAttr pkgs.system flakeOutputs.checks;
    };
    expected = {
      hasPackages = true;
      hasBinCalcPackage = true;
      hasBinCalcDevColonPackage = false;
      hasBinCalcDevDashPackage = false;
      hasLibCalcPackage = false;
      hasLibCalcDevColonPackage = false;
      hasLibCalcLibDevColonPackage = false;
      hasLibCalcCheck = true;
      hasApps = true;
      hasBinCalcDevColonApp = false;
      hasBinCalcDevDashApp = false;
      hasLockApp = true;
      hasDevShell = true;
      hasChecks = true;
    };
  };

  testMkFlakeOptionalPackages = {
    expr = let
      flakeOutputs = mkFlake {
        nixpkgs = mockNixpkgs;
        systems = [pkgs.system];
        root = ../tests/basic-workspace;
        createLibPackages = true;
        createDevPackages = true;
      };
      packages = flakeOutputs.packages.${pkgs.system};
      apps = flakeOutputs.apps.${pkgs.system};
    in {
      hasBinCalcPackage = builtins.hasAttr "bin-calc" packages;
      hasBinCalcDevPackage = builtins.hasAttr "bin-calc:dev" packages;
      hasLibCalcPackage = builtins.hasAttr "lib-calc" packages;
      hasLibCalcLibPackage = builtins.hasAttr "lib-calc-lib" packages;
      hasLibCalcDevPackage = builtins.hasAttr "lib-calc:dev" packages;
      hasLibCalcLibDevPackage = builtins.hasAttr "lib-calc-lib:dev" packages;
      hasBinCalcDevApp = builtins.hasAttr "bin-calc:dev" apps;
    };
    expected = {
      hasBinCalcPackage = true;
      hasBinCalcDevPackage = true;
      hasLibCalcPackage = true;
      hasLibCalcLibPackage = true;
      hasLibCalcDevPackage = true;
      hasLibCalcLibDevPackage = true;
      hasBinCalcDevApp = true;
    };
  };

  testMkFlakeDisableDevPackages = {
    expr = let
      flakeOutputs = mkFlake {
        nixpkgs = mockNixpkgs;
        systems = [pkgs.system];
        root = ../tests/basic-workspace;
        createLibPackages = true;
        createDevPackages = false;
      };
    in {
      hasBinCalcPackage = builtins.hasAttr "bin-calc" flakeOutputs.packages.${pkgs.system};
      hasBinCalcDevPackage = builtins.hasAttr "bin-calc:dev" flakeOutputs.packages.${pkgs.system};
      hasBinCalcDevApp = builtins.hasAttr "bin-calc:dev" flakeOutputs.apps.${pkgs.system};
      hasLibCalcPackage = builtins.hasAttr "lib-calc" flakeOutputs.packages.${pkgs.system};
      hasLibCalcDevPackage = builtins.hasAttr "lib-calc:dev" flakeOutputs.packages.${pkgs.system};
    };
    expected = {
      hasBinCalcPackage = true;
      hasBinCalcDevPackage = false;
      hasBinCalcDevApp = false;
      hasLibCalcPackage = true;
      hasLibCalcDevPackage = false;
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
}
