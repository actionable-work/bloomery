{
  description = "bloomery test workspace (flake-parts module)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    bloomery.url = "path:../..";
  };

  outputs = inputs @ {
    flake-parts,
    bloomery,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      imports = [
        bloomery.flakeModules.default
      ];
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      perSystem = {
        bloomery.workspace = {
          root = ./.;
        };
      };
    };
}
