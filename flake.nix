{
  description = "bloomery: Pure Nix Rust builder without Cargo, pulling dependencies directly from Cargo.lock";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    flake-parts,
    nixpkgs,
    treefmt-nix,
    ...
  }: let
    inherit (nixpkgs) lib;
    systems = import ./nix/systems.nix;

    bloomeryFor = system: let
      pkgs = nixpkgs.legacyPackages.${system};
    in
      import ./lib {
        inherit pkgs;
        inherit (pkgs) lib;
      };

    mkFlake = import ./lib/mk-flake.nix {
      bloomeryLib = {
        pkgs,
        lib ? pkgs.lib,
      }:
        import ./lib {inherit pkgs lib;};
    };
  in
    (mkFlake {
      inherit nixpkgs systems;
      root = ./.;
      extraOutputs = {
        eachSystem,
        perSystemWorkspace,
      }: {
        apps = eachSystem (system: let
          rootWorkspace = perSystemWorkspace.${system};
        in
          rootWorkspace.apps);

        formatter = eachSystem (system: let
          pkgs = nixpkgs.legacyPackages.${system};
          treefmtModule = treefmt-nix.lib.evalModule pkgs ./nix/lib/treefmt-config.nix;
        in
          treefmtModule.config.build.wrapper);

        checks = eachSystem (system: let
          pkgs = nixpkgs.legacyPackages.${system};
          bloomery = bloomeryFor system;
          rootWorkspace = perSystemWorkspace.${system};
          docsPackage = rootWorkspace.packages."bloomery-docs";
          treefmtModule = treefmt-nix.lib.evalModule pkgs ./nix/lib/treefmt-config.nix;
          testFlakeChecks = import ./nix/test-flake-checks.nix {
            inherit lib pkgs nixpkgs flake-parts system;
            bloomery = self;
          };
          rootChecks = import ./nix/checks {
            inherit lib pkgs bloomery docsPackage;
            root = ./.;
            workspace = rootWorkspace;
            treefmt = treefmtModule.config;
          };
        in
          rootChecks
          // testFlakeChecks);
      };
    })
    // {
      inherit mkFlake;
    };
}
