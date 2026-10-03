{
  pkgs,
  lib ? pkgs.lib,
}: let
  mkFlake = import ./mk-flake.nix {
    bloomeryLib = {
      pkgs,
      lib,
    }:
      import ./. {
        inherit pkgs lib;
        bloomeryPackageForSystem = _system: pkgs.hello;
      };
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
      hasLibCalcLibPackage = builtins.hasAttr "lib-calc:lib" flakeOutputs.packages.${pkgs.system};
      hasLibCalcDevColonPackage = builtins.hasAttr "lib-calc:dev" flakeOutputs.packages.${pkgs.system};
      hasLibCalcLibDevColonPackage = builtins.hasAttr "lib-calc:lib:dev" flakeOutputs.packages.${pkgs.system};
      hasLibCalcCheck = builtins.hasAttr "lib-calc:lib" flakeOutputs.checks.${pkgs.system};
      hasApps = builtins.hasAttr pkgs.system flakeOutputs.apps;
      hasBinCalcDevColonApp = builtins.hasAttr "bin-calc:dev" flakeOutputs.apps.${pkgs.system};
      hasBinCalcDevDashApp = builtins.hasAttr "bin-calc-dev" flakeOutputs.apps.${pkgs.system};
      hasLibCalcDocApp = builtins.hasAttr "lib-calc:doc" flakeOutputs.apps.${pkgs.system};
      hasLibCalcDashDocApp = builtins.hasAttr "lib-calc-doc" flakeOutputs.apps.${pkgs.system};
      hasLockApp = builtins.hasAttr "lock" flakeOutputs.apps.${pkgs.system};
      hasSyncApp = builtins.hasAttr "sync" flakeOutputs.apps.${pkgs.system};
      hasBinCalcApp = builtins.hasAttr "bin-calc" flakeOutputs.apps.${pkgs.system};
      hasDefaultApp = builtins.hasAttr "default" flakeOutputs.apps.${pkgs.system};
      hasDefaultPackage = builtins.hasAttr "default" flakeOutputs.packages.${pkgs.system};
      hasDevShell = builtins.hasAttr "default" flakeOutputs.devShells.${pkgs.system};
      hasChecks = builtins.hasAttr pkgs.system flakeOutputs.checks;
      hasBloomeryCheck = builtins.hasAttr "bloomery:check" flakeOutputs.checks.${pkgs.system};
    };
    expected = {
      hasPackages = true;
      hasBinCalcPackage = true;
      hasBinCalcDevColonPackage = false;
      hasBinCalcDevDashPackage = false;
      hasLibCalcPackage = false;
      hasLibCalcLibPackage = false;
      hasLibCalcDevColonPackage = false;
      hasLibCalcLibDevColonPackage = false;
      hasLibCalcCheck = true;
      hasApps = true;
      hasBinCalcDevColonApp = false;
      hasBinCalcDevDashApp = false;
      hasLibCalcDocApp = true;
      hasLibCalcDashDocApp = false;
      hasLockApp = false;
      hasSyncApp = false;
      hasBinCalcApp = true;
      hasDefaultApp = true;
      hasDefaultPackage = true;
      hasDevShell = true;
      hasChecks = true;
      hasBloomeryCheck = true;
    };
  };

  testMkFlakeExplicitMainBinary = {
    expr = let
      flakeOutputs = mkFlake {
        nixpkgs = mockNixpkgs;
        systems = [pkgs.system];
        root = ../.;
        createLibPackages = false;
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
      hasBloomeryCheck = true;
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
      hasLibCalcLibPackage = builtins.hasAttr "lib-calc:lib" packages;
      hasLibCalcDevPackage = builtins.hasAttr "lib-calc:dev" packages;
      hasLibCalcLibDevPackage = builtins.hasAttr "lib-calc:lib:dev" packages;
      hasBinCalcDevApp = builtins.hasAttr "bin-calc:dev" apps;
      hasLibCalcDocApp = builtins.hasAttr "lib-calc:doc" apps;
      hasLibCalcDashDocApp = builtins.hasAttr "lib-calc-doc" apps;
    };
    expected = {
      hasBinCalcPackage = true;
      hasBinCalcDevPackage = false;
      hasLibCalcPackage = false;
      hasLibCalcLibPackage = true;
      hasLibCalcDevPackage = false;
      hasLibCalcLibDevPackage = false;
      hasBinCalcDevApp = true;
      hasLibCalcDocApp = true;
      hasLibCalcDashDocApp = false;
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
      hasLibCalcLibPackage = builtins.hasAttr "lib-calc:lib" flakeOutputs.packages.${pkgs.system};
      hasLibCalcDevPackage = builtins.hasAttr "lib-calc:dev" flakeOutputs.packages.${pkgs.system};
      hasDefaultApp = builtins.hasAttr "default" flakeOutputs.apps.${pkgs.system};
      hasDefaultPackage = builtins.hasAttr "default" flakeOutputs.packages.${pkgs.system};
    };
    expected = {
      hasBinCalcPackage = true;
      hasBinCalcDevPackage = false;
      hasBinCalcDevApp = false;
      hasLibCalcPackage = false;
      hasLibCalcLibPackage = true;
      hasLibCalcDevPackage = false;
      hasDefaultApp = true;
      hasDefaultPackage = true;
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
