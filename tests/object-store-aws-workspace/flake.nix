{
  description = "Public object_store AWS build compatibility";
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
