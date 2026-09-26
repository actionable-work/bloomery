{ pkgs, lib ? pkgs.lib, cratesIoIndex ? null }:

rec {
  # Submodules
  builders = import ./builders { inherit pkgs lib; };
  workspace = import ./workspace { inherit lib; };
  profile = import ./profile { inherit lib; };
  overrides = import ./overrides { inherit pkgs lib; };
  docs = import ./docs { inherit pkgs lib; };
  lock = import ./lock { inherit pkgs lib; };
  tests = import ./tests.nix { inherit pkgs lib; };

  # High-level workspace builder
  mkWorkspace = import ./mk-workspace.nix { inherit pkgs lib cratesIoIndex; };

  # Convenience re-exports
  inherit (builders)
    buildCrateWith
    buildBinWith
    testCrateWith
    clippyCrateWith
    docCrateWith
    doctestCrateWith;

  inherit (workspace)
    discoverWorkspaceCrates
    extractFeaturesFromToml
    unifyWorkspaceFeatures
    resolveFeatures
    parseLock;

  inherit (profile)
    profileOptionModule
    profileType
    evalProfile
    profileToRustcFlags;

  types = profile;
  defaultOverrides = overrides;
}
