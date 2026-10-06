{
  description = "naersk benchmark builder";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    workspace.url = "path:../../fixtures/standard";
  };

  outputs = {
    nixpkgs,
    naersk,
    workspace,
    ...
  }: let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
    naerskLib = pkgs.callPackage naersk {};
    app = naerskLib.buildPackage {src = workspace.lib.src;};
    test = naerskLib.buildPackage {
      src = workspace.lib.src;
      doCheck = true;
    };
  in {
    packages.${system} = {default = app;};
    checks.${system} = {inherit test;};
  };
}
