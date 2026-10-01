{
  description = "bloomery test workspace (custom pkgs via mkLib constructor)";

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
      workspace = system: let
        # Example of customized nixpkgs instance with custom overlays
        pkgs = import nixpkgs {
          inherit system;
          overlays = [];
        };
        # Construct bloomery using mkLib
        bl = bloomery.mkLib pkgs;
      in
        bl.mkWorkspace {
          root = ./.;
          profile = {
            optLevel = 2;
          };
        };
      extraChecks = import ./checks.nix {inherit nixpkgs;};
    };
}
