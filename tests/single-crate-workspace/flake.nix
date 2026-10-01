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
    import ../workspace-flake.nix {
      inherit nixpkgs;
      workspace = system:
        bloomery.lib.${system}.mkWorkspace {
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
        };
      extraChecks = import ./checks.nix {inherit nixpkgs;};
    };
}
