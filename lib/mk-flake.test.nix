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
      hasApps = builtins.hasAttr pkgs.system flakeOutputs.apps;
      hasLockApp = builtins.hasAttr "lock" flakeOutputs.apps.${pkgs.system};
      hasDevShell = builtins.hasAttr "default" flakeOutputs.devShells.${pkgs.system};
      hasChecks = builtins.hasAttr pkgs.system flakeOutputs.checks;
    };
    expected = {
      hasPackages = true;
      hasBinCalcPackage = true;
      hasApps = true;
      hasLockApp = true;
      hasDevShell = true;
      hasChecks = true;
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
