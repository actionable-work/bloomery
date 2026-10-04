{
  description = "bloomery single-crate workspace with overrides";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    bloomery.url = "path:../..";
  };

  outputs = {
    nixpkgs,
    bloomery,
    ...
  }:
    bloomery.mkFlake {
      inherit nixpkgs;
      root = ./.;
      overrides = {
        single-crate-app = {
          env = {
            TOP_LEVEL_OVERRIDE_VAR = "injected_from_single_flake";
          };
          rustcFlags = [
            "--cfg=single_toplevel_override"
          ];
        };
      };
      extraOutputs = {
        eachSystem,
        perSystemWorkspace,
      }: {
        checks = eachSystem (
          system:
            perSystemWorkspace.${system}.checks
            // ((import ./checks.nix {inherit nixpkgs;}) system perSystemWorkspace.${system})
        );
      };
    };
}
