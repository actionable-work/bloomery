{
  description = "nixpkgs rustPlatform benchmark builder";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    workspace.url = "path:../../fixtures/standard";
  };

  outputs = {
    nixpkgs,
    workspace,
    ...
  }: let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
    src = workspace.lib.src;
    lockFile = "${src}/Cargo.lock";
    app = pkgs.rustPlatform.buildRustPackage {
      pname = "app";
      version = "0.1.0";
      inherit src;
      cargoLock = {inherit lockFile;};
      cargoBuildFlags = ["--bin" "app"];
    };
    test = pkgs.rustPlatform.buildRustPackage {
      pname = "workspace-tests";
      version = "0.1.0";
      inherit src;
      cargoLock = {inherit lockFile;};
      doCheck = true;
      cargoTestFlags = ["--workspace"];
    };
  in {
    packages.${system} = {default = app;};
    checks.${system} = {inherit test;};
  };
}
