# Evaluation-time unit tests for check generation, target discovery, and
# compiler flags.
{
  pkgs,
  lib,
}: let
  workspaceTests = import ../../../lib/mk-workspace.test.nix {inherit pkgs lib;};
  builderTests = import ../../../lib/builders/builders.test.nix {inherit pkgs lib;};
  targetTests = import ../../../lib/workspace/cargo-targets.test.nix {inherit lib;};
  failures = lib.runTests (
    {
      inherit (workspaceTests) testUnusedDependencyChecksAreGeneratedAndGated testUnusedDependencyChecksDeclareEdgesAndTargets;
      inherit (builderTests) testUnusedDependencyBuilderCompilesWithResolvedFeatures testUnusedDependencyBuilderCapsLintLevels;
    }
    // targetTests
  );
in
  pkgs.runCommand "bloomery-unused-dependencies-units" {
    passthru.bloomery = [
      "NIXLIB-GRAPH-UNUSED-001"
      "NIXLIB-GRAPH-UNUSED-009"
      "NIXLIB-GRAPH-UNUSED-010"
      "NIXLIB-GRAPH-UNUSED-018"
      "NIXLIB-GRAPH-UNUSED-019"
    ];
    failures = builtins.toJSON failures;
  } ''
    if [ "$failures" != "[]" ]; then
      echo "Unused dependency unit tests failed: $failures" >&2
      exit 1
    fi
    touch "$out"
  ''
