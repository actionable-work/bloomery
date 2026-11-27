# Evaluation-time unit tests for configuration mapping and lockless
# workspaces.
{
  pkgs,
  lib,
}: let
  workspaceTests = import ../../../lib/mk-workspace.test.nix {inherit pkgs lib;};
  configTests = import ../../../lib/build-config.test.nix {inherit pkgs lib;};
  failures = lib.runTests {
    inherit (workspaceTests) testUnusedDependencyChecksResolveWithoutLock;
    inherit (configTests) testUnusedDependenciesCompilerMapsToWorkspaceOptions testUnusedDependenciesCompilerRejectsNonBoolean;
  };
in
  pkgs.runCommand "bloomery-unused-dependencies-loading" {
    passthru.bloomery = [
      "NIXLIB-GRAPH-UNUSED-020"
      "PARSER-CONFIGURATION-UNUSED-007"
      "PARSER-CONFIGURATION-UNUSED-008"
    ];
    failures = builtins.toJSON failures;
  } ''
    if [ "$failures" != "[]" ]; then
      echo "Unused dependency loading tests failed: $failures" >&2
      exit 1
    fi
    touch "$out"
  ''
