{
  description = "{{project}}: a Bloomery Rust workspace";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    bloomery = {
      url = "github:actionable-work/bloomery";
      inputs.nixpkgs.follows = "nixpkgs";
    };
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
