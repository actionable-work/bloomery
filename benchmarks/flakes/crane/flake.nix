{
  description = "crane benchmark builder";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    workspace.url = "path:../../fixtures/standard";
  };

  outputs = {
    nixpkgs,
    crane,
    workspace,
    ...
  }: let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
    craneLib = crane.mkLib pkgs;
    src = craneLib.cleanCargoSource workspace.lib.src;
    common = {inherit src;};
    cargoArtifacts = craneLib.buildDepsOnly common;
    app = craneLib.buildPackage (common // {inherit cargoArtifacts;});
    test = craneLib.cargoTest (common // {inherit cargoArtifacts;});
    clippy = craneLib.cargoClippy (common
      // {
        inherit cargoArtifacts;
        cargoClippyExtraArgs = "--all-targets -- --deny warnings";
      });
    doc = craneLib.cargoDoc (common // {inherit cargoArtifacts;});
  in {
    packages.${system} = {default = app;};
    checks.${system} = {inherit clippy doc test;};
  };
}
