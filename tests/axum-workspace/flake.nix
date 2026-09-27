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
    workspaces = eachSystem (
      system:
        bloomery.lib.${system}.mkWorkspace {
          root = ./.;
          profile = {
            optLevel = 3;
            codegenUnits = 16;
          };
          toolchain = {
            linker = "lld";
          };
          flags = {
            rustc = ["-Copt-level=3"];
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
