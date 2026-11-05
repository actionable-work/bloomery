{
  description = "Custom Cargo library names and dependency aliases";
  inputs = {
    nixpkgs.follows = "bloomery/nixpkgs";
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
      extraOutputs = import ./checks.nix {inherit nixpkgs;};
    };
}
