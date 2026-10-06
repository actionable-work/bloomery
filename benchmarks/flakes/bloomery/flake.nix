{
  description = "Bloomery benchmark builder";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    bloomery = {
      url = "path:../../../";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    workspace.url = "path:../../fixtures/standard";
  };

  outputs = {
    nixpkgs,
    bloomery,
    workspace,
    ...
  }:
    bloomery.mkFlake {
      inherit nixpkgs;
      root = workspace.lib.src;
      systems = ["x86_64-linux"];
    };
}
