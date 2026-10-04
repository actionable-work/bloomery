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
    bloomery.mkFlake {
      inherit nixpkgs;
      root = ./.;
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
