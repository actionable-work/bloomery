{
  pkgs,
  lib ? pkgs.lib,
}: let
  profile = import ./profile {inherit lib;};
  workspace = import ./workspace {inherit pkgs lib;};
  mkFlakeTests = import ./mk-flake.test.nix {inherit pkgs lib;};
  mkWorkspaceTests = import ./mk-workspace.test.nix {inherit pkgs lib;};
  flakeModuleTests = import ./modules/flake-module.test.nix {inherit pkgs lib;};
  builderTests = import ./builders/builders.test.nix {inherit pkgs lib;};

  # Aggregate all colocated unit tests
  testCases = profile.tests // workspace.tests // mkFlakeTests // mkWorkspaceTests // flakeModuleTests // builderTests;

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
