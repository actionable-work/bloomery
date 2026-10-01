{
  description = "bloomery test workspace";

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
      createLibPackages = true;
      createDevPackages = true;
      extraOutputs = import ./checks.nix {inherit nixpkgs;};
    };
}
