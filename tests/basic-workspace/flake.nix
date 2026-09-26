{
  description = "bloomery test workspace";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    bloomery.url = "path:../..";
  };

  outputs = { self, nixpkgs, bloomery }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      eachSystem = nixpkgs.lib.genAttrs systems;
    in {
      packages = eachSystem (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          bl = bloomery.mkLib.${system};
          workspace = bl.mkWorkspace {
            root = ./.;
          };
        in workspace.packages
      );

      apps = eachSystem (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          bl = bloomery.mkLib.${system};
          workspace = bl.mkWorkspace {
            root = ./.;
          };
        in workspace.apps
      );

      checks = eachSystem (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          bl = bloomery.mkLib.${system};
          workspace = bl.mkWorkspace {
            root = ./.;
          };
        in workspace.checks
      );
    };
}
