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
  repositoryLayoutTests = import ./repository-layout.test.nix {inherit lib;};

  # Aggregate all colocated unit tests
  testCases = profile.tests // workspace.tests // mkFlakeTests // mkWorkspaceTests // flakeModuleTests // builderTests // repositoryLayoutTests;

  failedTests = lib.runTests testCases;
in {
  inherit testCases failedTests;

  # Derivation for flake check
  check =
    pkgs.runCommand "bloomery-unit-tests" {
      passthru = {
        inherit failedTests;
        bloomery = [
          "REPOSITORY-LAYOUT-STRUCTURE-001"
          "REPOSITORY-LAYOUT-STRUCTURE-002"
          "REPOSITORY-LAYOUT-STRUCTURE-003"
          "REPOSITORY-LAYOUT-STRUCTURE-004"
          "REPOSITORY-LAYOUT-STRUCTURE-005"
          "REPOSITORY-LAYOUT-STRUCTURE-006"
        ];
      };
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
