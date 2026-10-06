{
  description = "crate2nix benchmark builder";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crate2nix = {
      url = "github:nix-community/crate2nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    workspace.url = "path:../../fixtures/standard";
  };

  outputs = {
    nixpkgs,
    workspace,
    ...
  }: let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
    cargoNix = import (workspace.lib.src + "/crate2nix.Cargo.gen") {inherit pkgs;};
    app = cargoNix.workspaceMembers.app.build;
  in {
    packages.${system} = {default = app;};
    checks.${system} = {
      app = cargoNix.workspaceMembers.app.build;
    };
  };
}
