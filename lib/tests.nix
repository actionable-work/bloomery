{
  pkgs,
  lib ? pkgs.lib,
}: let
  profile = import ./profile {inherit lib;};
  workspace = import ./workspace {inherit pkgs lib;};
  mkFlakeTests = import ./mk-flake.test.nix {inherit pkgs lib;};
  buildConfigTests = import ./build-config.test.nix {inherit pkgs lib;};
  mkWorkspaceTests = import ./mk-workspace.test.nix {inherit pkgs lib;};
  flakeModuleTests = import ./modules/flake-module.test.nix {inherit pkgs lib;};
  builderTests = import ./builders/builders.test.nix {inherit pkgs lib;};
  repositoryLayoutTests = import ./repository-layout.test.nix {inherit lib;};
  templatesSourceTests = import ./templates-source.test.nix {inherit pkgs lib;};
  sourceIsolationTests = import ./source-isolation.test.nix {inherit pkgs lib;};
  graphTests = import ./graph.test.nix {inherit pkgs lib;};

  # Aggregate all colocated unit tests
  testCases =
    profile.tests
    // workspace.tests
    // mkFlakeTests
    // buildConfigTests
    // mkWorkspaceTests
    // flakeModuleTests
    // builderTests
    // repositoryLayoutTests
    // templatesSourceTests
    // sourceIsolationTests
    // graphTests;

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
          "NIX-SOURCES-IDENTITY-001"
          "NIX-SOURCES-IDENTITY-002"
          "NIX-SOURCES-IDENTITY-003"
          "NIX-SOURCES-IDENTITY-004"
          "NIX-SOURCES-IDENTITY-005"
          "NIX-SOURCES-IDENTITY-006"
          "NIX-SOURCES-IDENTITY-007"
          "NIX-SOURCES-IDENTITY-008"
          "NIX-SOURCES-IDENTITY-009"
          "NIX-SOURCES-IDENTITY-010"
          "NIXLIB-FLAKE-ENTRYPOINTS-003"
          "NIXLIB-FLAKE-ENTRYPOINTS-004"
          "NIXLIB-FLAKE-ENTRYPOINTS-005"
          "NIXLIB-FLAKE-ENTRYPOINTS-006"
          "NIXLIB-FLAKE-ENTRYPOINTS-007"
          "NIXLIB-FLAKE-ENTRYPOINTS-008"
          "NIXLIB-FLAKE-ENTRYPOINTS-009"
          "NIXLIB-FLAKE-ENTRYPOINTS-012"
          "NIXLIB-FLAKE-ENTRYPOINTS-013"
          "NIXLIB-FLAKE-ENTRYPOINTS-014"
          "NIXLIB-FLAKE-ENTRYPOINTS-016"
          "NIXLIB-FLAKE-CONFIGURATION-001"
          "NIXLIB-FLAKE-CONFIGURATION-002"
          "NIXLIB-FLAKE-CONFIGURATION-003"
          "NIXLIB-FLAKE-CONFIGURATION-004"
          "NIXLIB-FLAKE-CONFIGURATION-005"
          "NIXLIB-FLAKE-CONFIGURATION-006"
          "NIXLIB-FLAKE-CONFIGURATION-007"
          "NIXLIB-FLAKE-CONFIGURATION-008"
          "NIXLIB-FLAKE-CONFIGURATION-009"
          "NIXLIB-FLAKE-CONFIGURATION-010"
          "NIXLIB-FLAKE-CONFIGURATION-011"
          "NIXLIB-FLAKE-CONFIGURATION-012"
          "NIXLIB-FLAKE-CONFIGURATION-013"
          "NIXLIB-FLAKE-CONFIGURATION-014"
          "NIXLIB-FLAKE-CONFIGURATION-015"
          "NIXLIB-FLAKE-CONFIGURATION-016"
          "NIXLIB-FLAKE-CONFIGURATION-017"
          "NIXLIB-FLAKE-CONFIGURATION-018"
          "NIXLIB-FLAKE-OPTIONS-001"
          "NIXLIB-FLAKE-OPTIONS-002"
          "NIXLIB-FLAKE-OPTIONS-003"
          "NIXLIB-FLAKE-OPTIONS-004"
          "NIXLIB-FLAKE-OPTIONS-005"
          "NIXLIB-FLAKE-OPTIONS-006"
          "NIXLIB-FLAKE-OPTIONS-007"
          "NIXLIB-FLAKE-OPTIONS-008"
          "NIXLIB-FLAKE-OPTIONS-009"
          "NIXLIB-FLAKE-OPTIONS-010"
          "NIXLIB-FLAKE-OPTIONS-011"
          "NIXLIB-FLAKE-OPTIONS-012"
          "NIXLIB-FLAKE-OPTIONS-013"
          "NIXLIB-FLAKE-OPTIONS-014"
          "NIXLIB-FLAKE-OPTIONS-015"
          "NIXLIB-FLAKE-OPTIONS-016"
          "NIXLIB-FLAKE-OPTIONS-017"
          "NIXLIB-FLAKE-OPTIONS-018"
          "NIXLIB-FLAKE-OUTPUTS-003"
          "NIXLIB-FLAKE-OUTPUTS-009"
          "NIXLIB-FLAKE-OUTPUTS-011"
          "NIXLIB-FLAKE-OUTPUTS-013"
          "NIXLIB-FLAKE-OUTPUTS-014"
          "NIXLIB-FLAKE-OUTPUTS-015"
          "NIXLIB-FLAKE-OUTPUTS-016"
          "NIXLIB-FLAKE-OUTPUTS-017"
          "NIXLIB-FLAKE-OUTPUTS-018"
          "NIXLIB-FLAKE-OUTPUTS-019"
          "NIXLIB-FLAKE-OUTPUTS-020"
          "NIXLIB-FLAKE-OUTPUTS-021"
          "NIXLIB-FLAKE-OUTPUTS-022"
          "NIXLIB-GRAPH-RESOLUTION-001"
          "NIXLIB-GRAPH-RESOLUTION-002"
          "NIXLIB-GRAPH-RESOLUTION-003"
          "NIXLIB-GRAPH-RESOLUTION-004"
          "NIXLIB-GRAPH-RESOLUTION-005"
          "NIXLIB-GRAPH-RESOLUTION-006"
          "NIXLIB-GRAPH-RESOLUTION-007"
          "NIXLIB-GRAPH-RESOLUTION-008"
          "NIXLIB-GRAPH-RESOLUTION-009"
          "NIXLIB-GRAPH-RESOLUTION-010"
          "NIXLIB-GRAPH-RESOLUTION-011"
          "NIXLIB-GRAPH-RESOLUTION-012"
          "NIXLIB-GRAPH-RESOLUTION-014"
          "NIXLIB-GRAPH-RESOLUTION-015"
          "NIXLIB-GRAPH-RESOLUTION-016"
          "NIXLIB-GRAPH-RESOLUTION-017"
          "NIXLIB-GRAPH-RESOLUTION-018"
          "NIXLIB-GRAPH-RESOLUTION-019"
          "NIXLIB-GRAPH-RESOLUTION-020"
          "NIXLIB-GRAPH-DERIVATIONS-001"
          "NIXLIB-GRAPH-DERIVATIONS-003"
          "NIXLIB-GRAPH-DERIVATIONS-004"
          "NIXLIB-GRAPH-DERIVATIONS-005"
          "NIXLIB-GRAPH-DERIVATIONS-006"
          "NIXLIB-GRAPH-DERIVATIONS-007"
          "NIXLIB-GRAPH-DERIVATIONS-008"
          "NIXLIB-GRAPH-DERIVATIONS-010"
          "NIXLIB-GRAPH-DERIVATIONS-011"
          "NIXLIB-GRAPH-DERIVATIONS-014"
          "NIXLIB-GRAPH-DERIVATIONS-016"
          "NIXLIB-GRAPH-DERIVATIONS-017"
          "NIXLIB-GRAPH-DERIVATIONS-018"
          "NIXLIB-GRAPH-DERIVATIONS-019"
          "NIXLIB-GRAPH-DERIVATIONS-020"
          "NIXLIB-GRAPH-DERIVATIONS-021"
          "NIXLIB-GRAPH-MANIFEST-001"
          "NIXLIB-GRAPH-MANIFEST-002"
          "NIXLIB-GRAPH-MANIFEST-003"
          "NIXLIB-GRAPH-MANIFEST-004"
          "NIXLIB-GRAPH-MANIFEST-005"
          "NIXLIB-GRAPH-MANIFEST-006"
          "NIXLIB-GRAPH-MANIFEST-007"
          "NIX-SOURCES-FILESETS-011"
          "CLI-INIT-TEMPLATES-008"
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
