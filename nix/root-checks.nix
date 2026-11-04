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
    "template-workspaces" =
      pkgs.runCommand "bloomery-template-workspaces-check" {
        passthru.bloomery = ["CLI-INIT-TEMPLATES-012"];
      } ''
        ${lib.concatMapStringsSep "\n" (check: "test -e ${check}") templateWorkspaceChecks}
        mkdir "$out"
        echo "passed" > "$out/success"
      '';
  }
