{ lib }:

let
  discover = import ./discover.nix { inherit lib; };
  features = import ./features.nix { inherit lib; };
  parseLock = import ./parse-lock.nix { inherit lib; };
  discoverTests = import ./discover.test.nix { inherit lib; };
  featuresTests = import ./features.test.nix { inherit lib; };
  parseLockTests = import ./parse-lock.test.nix { inherit lib; };
in {
  inherit (discover) discoverWorkspaceCrates;
  inherit (features) extractFeaturesFromToml unifyWorkspaceFeatures resolveFeatures;
  inherit (parseLock) parseDepString parseLock;
  tests = discoverTests // featuresTests // parseLockTests;
}
