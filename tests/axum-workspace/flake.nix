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
  }:
    import ../workspace-flake.nix {
      inherit nixpkgs;
      workspace = system:
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
        };
      extraChecks = import ./checks.nix {inherit nixpkgs;};
    };
}
