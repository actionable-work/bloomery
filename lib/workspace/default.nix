{
  lib,
  pkgs ? null,
}: let
  discover = import ./discover.nix {inherit lib;};
  features = import ./features.nix {inherit lib;};
  parseLock = import ./parse-lock.nix {inherit lib;};
  options =
    if pkgs != null
    then import ./options.nix {inherit pkgs lib;}
    else null;
  discoverTests = import ./discover.test.nix {inherit lib;};
  featuresTests = import ./features.test.nix {inherit lib;};
  parseLockTests = import ./parse-lock.test.nix {inherit lib;};
  optionsTests =
    if pkgs != null
    then import ./options.test.nix {inherit pkgs lib;}
    else {};
in {
  inherit (discover) discoverWorkspaceCrates;
  inherit (features) extractFeaturesFromToml unifyWorkspaceFeatures resolveFeatures;
  inherit (parseLock) parseDepString parseLock;
  inherit options;
  tests = discoverTests // featuresTests // parseLockTests // optionsTests;
}
