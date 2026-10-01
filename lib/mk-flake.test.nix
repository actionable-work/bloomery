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
  testMkFlakeBasic = {
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
      hasBinCalcDevColonPackage = true;
      hasBinCalcDevDashPackage = false;
      hasLibCalcPackage = true;
      hasLibCalcDevColonPackage = true;
      hasLibCalcLibDevColonPackage = true;
      hasApps = true;
      hasBinCalcDevColonApp = true;
      hasBinCalcDevDashApp = false;
      hasLockApp = true;
      hasDevShell = true;
      hasChecks = true;
    };
  };

  testMkFlakeDisableDevPackages = {
    expr = let
      flakeOutputs = mkFlake {
        nixpkgs = mockNixpkgs;
        systems = [pkgs.system];
        root = ../tests/basic-workspace;
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
