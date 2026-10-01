{
  description = "Edge cases test workspace";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    bloomery.url = "path:../..";
  };

  outputs = {
    self,
    nixpkgs,
    bloomery,
    ...
  }:
    bloomery.mkFlake {
      inherit self nixpkgs;
      root = ./.;
    };
}
