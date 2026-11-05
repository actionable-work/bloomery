{
  lib,
  nixpkgs,
  self,
  bloomeryFor,
  perSystemWorkspace,
  system,
}: let
  pkgs = nixpkgs.legacyPackages.${system};
  bloomery = bloomeryFor system;
  rootWorkspace = perSystemWorkspace.${system};
  docsPackage = rootWorkspace.packages."bloomery-docs";
  templateWorkspaceChecks = map (name: self.checks.${system}.${name}) [
    "template-basic:checks"
    "template-axum:checks"
    "template-topcoat:checks"
  ];
in
  (import ./checks {
    inherit lib pkgs nixpkgs bloomery docsPackage system;
    root = ../.;
    workspace = rootWorkspace;
    treefmt = rootWorkspace.formatterConfig;
  })
  // (import ./interface-checks.nix {
    inherit lib pkgs system;
    bloomery = self;
  })
  // {
    "optimization-smoke" = bloomery.tests.optimizedBinarySmoke;
    "naming-public-compatibility" =
      pkgs.runCommand "bloomery-naming-public-compatibility" {
        passthru.bloomery = ["NIXLIB-GRAPH-NAMING-026"];
      } ''
        ${lib.concatMapStringsSep "\n" (name: "test -e ${self.checks.${system}.${name}}") [
          "basic-workspace:checks"
          "flake-parts-workspace:checks"
          "single-crate-workspace:checks"
          "library-only-workspace:checks"
          "nixlib-workspace:checks"
          "interface:exports"
          "core:unit-tests"
          "library-names-workspace:checks"
        ]}
        mkdir "$out"
      '';
    "template-workspaces" =
      pkgs.runCommand "bloomery-template-workspaces-check" {
        passthru.bloomery = ["CLI-INIT-TEMPLATES-012"];
      } ''
        ${lib.concatMapStringsSep "\n" (check: "test -e ${check}") templateWorkspaceChecks}
        mkdir "$out"
        echo "passed" > "$out/success"
      '';
  }
