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
    import ../workspace-flake.nix {
      inherit nixpkgs;
      workspace = system:
        bloomery.lib.${system}.mkWorkspace {
          root = ./.;
          # Top-level overrides merge with the member's colocated overrides.nix.
          overrides = {
            bin-calc = {
              env = {
                TOP_LEVEL_OVERRIDE_VAR = "injected_from_flake_nix";
              };
              rustcFlags = [
                "--cfg=bloomery_toplevel_override"
              ];
            };
          };
        };
      extraChecks = import ./checks.nix {inherit nixpkgs;};
    };
}
