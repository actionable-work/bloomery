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
  }: let
    systems = [
      "x86_64-linux"
      "aarch64-linux"
      "aarch64-darwin"
    ];
    eachSystem = nixpkgs.lib.genAttrs systems;
    workspaces = eachSystem (
      system:
        bloomery.lib.${system}.mkWorkspace {
          root = ./.;
          # Top-level overrides merge with colocated overrides.nix
          overrides = {
            overrides-app = {
              env = {
                TOP_LEVEL_OVERRIDE_VAR = "injected_from_flake_nix";
              };
              rustcFlags = [
                "--cfg=bloomery_toplevel_overrides_validated"
              ];
            };
          };
        }
    );
  in {
    packages = eachSystem (system: workspaces.${system}.packages);
    apps = eachSystem (system: workspaces.${system}.apps);
    checks = eachSystem (system: workspaces.${system}.checks);
    devShells = eachSystem (system: {
      default = workspaces.${system}.devShell;
    });
  };
}
