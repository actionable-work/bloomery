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
in
  (import ./checks {
    inherit lib pkgs bloomery docsPackage;
    root = ../.;
    workspace = rootWorkspace;
    treefmt = rootWorkspace.formatterConfig;
  })
  // (import ./interface-checks.nix {
    inherit lib pkgs system;
    bloomery = self;
  })
