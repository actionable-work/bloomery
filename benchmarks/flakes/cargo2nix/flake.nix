{
  description = "cargo2nix benchmark builder";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    cargo2nix = {
      url = "github:cargo2nix/cargo2nix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-overlay.follows = "rust-overlay";
    };
    workspace.url = "path:../../fixtures/standard";
  };

  outputs = {
    nixpkgs,
    cargo2nix,
    workspace,
    ...
  }: let
    system = "x86_64-linux";
    pkgs = import nixpkgs {
      inherit system;
      overlays = [cargo2nix.overlays.default];
    };
    # Match the benchmark toolchain: use the pinned nixpkgs rustc version.
    rustToolchain = pkgs.rust-bin.stable.${pkgs.rustc.version}.minimal.override {
      extensions = ["rust-src"];
    };
    rustPkgs = pkgs.rustBuilder.makePackageSet {
      inherit rustToolchain;
      packageFun = import (workspace.lib.src + "/cargo2nix.Cargo.gen");
    };
    app = rustPkgs.workspace.app {};
  in {
    packages.${system} = {default = app;};
    checks.${system} = {
      app = rustPkgs.workspace.app {};
    };
  };
}
