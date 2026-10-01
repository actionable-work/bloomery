{
  description = "bloomery test workspace (custom pkgs via mkLib constructor)";

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
      system: let
        # Example of customized nixpkgs instance with custom overlays
        pkgs = import nixpkgs {
          inherit system;
          overlays = [];
        };
        # Construct bloomery using mkLib
        bl = bloomery.mkLib pkgs;
      in
        bl.mkWorkspace {
          root = ./.;
          profile = {
            optLevel = 2;
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
