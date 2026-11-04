{
  description = "template_axum: a Bloomery Axum server workspace";

  inputs = {
    bloomery.url = "github:actionable-work/bloomery";
    nixpkgs.follows = "bloomery/nixpkgs";
  };

  outputs = {
    nixpkgs,
    bloomery,
    ...
  }:
    bloomery.mkFlake {
      inherit nixpkgs;
      root = ./.;
    };
}
