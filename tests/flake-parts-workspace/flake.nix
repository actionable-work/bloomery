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
    nixpkgs,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      imports = [
        (import ../../lib/modules/flake-module.nix)
      ];
      systems = import ../../nix/systems.nix;
      perSystem = {
        config,
        system,
        ...
      }: let
        workspace = config.bloomery.outputs;
        extraChecks = import ./checks.nix {inherit nixpkgs;};
      in {
        bloomery.workspace = {
          root = ./.;
          checks.workspaceDependencies = false;
          checks.noDefaultFeatures = false;
        };
        checks = workspace.checks // extraChecks system workspace;
      };
    };
}
