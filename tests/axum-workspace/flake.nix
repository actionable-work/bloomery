{
  description = "bloomery axum and clap test workspace";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    bloomery.url = "path:../..";
  };

  outputs = {
    nixpkgs,
    bloomery,
    ...
  }: let
    systems = [
      "x86_64-linux"
      "aarch64-linux"
      "aarch64-darwin"
    ];
    eachSystem = nixpkgs.lib.genAttrs systems;
  in {
    packages = eachSystem (
      system: let
        bl = bloomery.mkLib.${system};
        workspace = bl.mkWorkspace {
          root = ./.;
        };
      in
        workspace.packages
    );

    apps = eachSystem (
      system: let
        bl = bloomery.mkLib.${system};
        workspace = bl.mkWorkspace {
          root = ./.;
        };
      in
        workspace.apps
    );

    checks = eachSystem (
      system: let
        bl = bloomery.mkLib.${system};
        workspace = bl.mkWorkspace {
          root = ./.;
        };
      in
        workspace.checks
    );
  };
}
