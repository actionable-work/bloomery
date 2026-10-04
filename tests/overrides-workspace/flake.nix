{
  description = "bloomery test workspace (validating colocated and flake overrides)";

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
      # Top-level overrides merge with the member's colocated overrides.nix.
      overrides = {
        bin-calc = {
          env = {
            TOP_LEVEL_OVERRIDE_VAR = "injected_from_flake_nix";
            SHARED_OVERRIDE_VAR = "flake";
          };
          rustcFlags = [
            "--cfg=bloomery_toplevel_override"
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
