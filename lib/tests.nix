{
  pkgs,
  lib ? pkgs.lib,
}: let
  profile = import ./profile {inherit lib;};
  workspace = import ./workspace {inherit lib;};

  # Aggregate all colocated unit tests
  testCases = profile.tests // workspace.tests;

  failedTests = lib.runTests testCases;
in {
  inherit testCases failedTests;

  # Derivation for flake check
  check =
    pkgs.runCommand "bloomery-unit-tests" {
      passthru = {inherit failedTests;};
    } ''
      ${
        if failedTests == []
        then ''
          echo "All ${toString (builtins.length (builtins.attrNames testCases))} colocated unit tests passed!"
          mkdir $out
          echo "passed" > $out/success
        ''
        else ''
          echo "Colocated unit tests failed!"
          echo '${builtins.toJSON failedTests}'
          exit 1
        ''
      }
    '';
}
