{
  lib,
  pkgs ? null,
}: let
  discover = import ./discover.nix {inherit lib;};
  features = import ./features.nix {inherit lib;};
  parseLock = import ./parse-lock.nix {inherit lib;};
  manifestPolicy = import ./manifest-policy.nix {inherit lib;};
  sources = import ./sources.nix {inherit lib pkgs;};
  options =
    if pkgs != null
    then import ./options.nix {inherit pkgs lib;}
    else null;
  discoverTests = import ./discover.test.nix {inherit lib;};
  featuresTests = import ./features.test.nix {inherit lib;};
  parseLockTests = import ./parse-lock.test.nix {inherit lib;};
  manifestPolicyTests = import ./manifest-policy.test.nix {inherit lib;};
  optionsTests =
    if pkgs != null
    then import ./options.test.nix {inherit pkgs lib;}
    else {};
in {
  inherit (discover) discoverWorkspaceCrates;
  inherit (features) extractFeaturesFromToml resolveFeatures;
  inherit (parseLock) parseDepString parseLock;
  inherit manifestPolicy;
  inherit sources;
  inherit options;
  tests = discoverTests // featuresTests // parseLockTests // manifestPolicyTests // optionsTests;
}
