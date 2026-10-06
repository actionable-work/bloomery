{
  pkgs,
  lib ? pkgs.lib,
  cratesIoIndex ? null,
  treefmtNix ? null,
  flakeParts ? null,
}: rec {
  # Submodules
  builders = import ./builders {inherit pkgs lib;};
  workspace = import ./workspace {inherit pkgs lib;};
  options = import ./workspace/options.nix {inherit pkgs lib;};
  profile = import ./profile {inherit lib;};
  overrides = import ./overrides {inherit pkgs lib;};
  docs = import ./docs {inherit pkgs lib;};
  tests = import ./tests.nix {inherit pkgs lib treefmtNix flakeParts;};
  modules = {
    flake = args:
      import ./modules/flake-module.nix args;
  };

  # High-level workspace builder
  mkWorkspace = import ./mk-workspace.nix {
    inherit pkgs lib cratesIoIndex treefmtNix;
  };

  # Zero-boilerplate flake builder
  mkFlake = import ./mk-flake.nix {
    bloomeryLib = {
      pkgs,
      lib ? pkgs.lib,
    }:
      import ./. {inherit pkgs lib treefmtNix;};
  };

  # Convenience re-exports
  inherit
    (builders)
    buildCrateWith
    buildBinWith
    testCrateWith
    clippyCrateWith
    docCrateWith
    doctestCrateWith
    ;

  inherit
    (workspace)
    discoverWorkspaceCrates
    extractFeaturesFromToml
    resolveFeatures
    parseLock
    ;

  inherit
    (profile)
    profileOptionModule
    profileType
    evalProfile
    profileToRustcFlags
    ;

  inherit
    (options)
    workspaceOptionModule
    evalWorkspaceOptions
    overrideOptionModule
    ;

  types = profile // {inherit (options) workspaceOptionModule overrideOptionModule;};
  defaultOverrides = overrides;
}
