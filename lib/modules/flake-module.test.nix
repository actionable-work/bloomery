{
  pkgs,
  lib ? pkgs.lib,
}: let
  flakeModule = args:
    import ./flake-module.nix (args // {bloomeryPackageForSystem = _system: pkgs.hello;});

  # Helper module providing standard flake-parts perSystem options
  mockFlakeParts = {
    options.perSystem = lib.mkOption {
      type = lib.types.submodule {
        config._module.args.pkgs = pkgs;
        options = {
          packages = lib.mkOption {
            type = lib.types.attrsOf lib.types.anything;
            default = {};
          };
          apps = lib.mkOption {
            type = lib.types.attrsOf lib.types.anything;
            default = {};
          };
          checks = lib.mkOption {
            type = lib.types.attrsOf lib.types.anything;
            default = {};
          };
          devShells = lib.mkOption {
            type = lib.types.attrsOf lib.types.anything;
            default = {};
          };
        };
      };
    };
  };

  # Test evaluating the module with bloomery.workspace configured
  evalResult = lib.evalModules {
    modules = [
      mockFlakeParts
      flakeModule
      {
        perSystem = {
          bloomery.workspace = {
            root = ../../tests/basic-workspace;
          };
        };
      }
    ];
  };

  # Test evaluating with bloomery.workspace disabled/null
  evalNullResult = lib.evalModules {
    modules = [
      mockFlakeParts
      flakeModule
      {
        perSystem = {};
      }
    ];
  };
in {
  testFlakeModuleOutputs = {
    expr = {
      hasPackages = builtins.hasAttr "bin-calc" (evalResult.config.perSystem.packages or {});
      hasLockApp = builtins.hasAttr "lock" (evalResult.config.perSystem.apps or {});
      hasDevShell = builtins.hasAttr "default" (evalResult.config.perSystem.devShells or {});
      hasChecks = builtins.hasAttr "bin-calc:bin" (evalResult.config.perSystem.checks or {});
    };
    expected = {
      hasPackages = true;
      hasLockApp = true;
      hasDevShell = true;
      hasChecks = true;
    };
  };

  testFlakeModuleNullWorkspace = {
    expr = {
      workspaceIsNull = evalNullResult.config.perSystem.bloomery.workspace == null;
    };
    expected = {
      workspaceIsNull = true;
    };
  };
}
