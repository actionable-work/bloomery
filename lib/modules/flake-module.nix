{
  lib,
  bloomeryPackageForSystem ? (_system: null),
  ...
} @ args: let
  flake-parts-lib = args.flake-parts-lib or null;
  perSystemModule = {
    config,
    pkgs,
    ...
  }: let
    optionsMod = import ../workspace/options.nix {inherit pkgs lib;};
  in {
    options.bloomery = {
      workspace = lib.mkOption {
        type = lib.types.nullOr (lib.types.submodule optionsMod.workspaceOptionModule);
        default = null;
        description = "Bloomery Rust workspace configuration.";
      };
      outputs = lib.mkOption {
        type = lib.types.attrs;
        internal = true;
        default = {};
        description = "Evaluated workspace outputs.";
      };
    };

    config = lib.mkIf (config.bloomery.workspace != null) (let
      bl = import ../. {inherit pkgs lib bloomeryPackageForSystem;};
      ws = bl.mkWorkspace config.bloomery.workspace;
    in {
      bloomery.outputs = ws;
      packages = lib.mkDefault ws.packages;
      apps = lib.mkDefault ws.apps;
      checks = lib.mkIf config.bloomery.workspace.checks.enable (lib.mkDefault ws.checks);
      devShells = lib.mkIf config.bloomery.workspace.devShell.enable {
        default = lib.mkDefault ws.devShell;
      };
    });
  };
in
  if flake-parts-lib != null && flake-parts-lib ? mkPerSystemOption
  then {
    options.perSystem = flake-parts-lib.mkPerSystemOption perSystemModule;
  }
  else {
    options.perSystem = lib.mkOption {
      type = lib.types.submodule perSystemModule;
    };
  }
