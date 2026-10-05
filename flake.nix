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

    bloomeryLib = {
      pkgs,
      lib ? pkgs.lib,
    }:
      import ./lib {
        inherit pkgs lib;
        treefmtNix = treefmt-nix;
      };

    bloomeryFor = system:
      bloomeryLib {pkgs = nixpkgs.legacyPackages.${system};};

    # The repository builds its own workspace without a pinned CLI so local
    # iteration is never shadowed by a prebuilt binary.
    mkSelfFlake = import ./lib/mk-flake.nix {
      inherit bloomeryLib;
    };

    # Consumers always receive the Bloomery CLI in the default development
    # shell, built from this flake's workspace with their nixpkgs.
    mkFlake = import ./lib/mk-flake.nix {
      inherit bloomeryLib;
      bloomeryCli = {
        pkgs,
        lib,
      }: let
        bl = import ./lib {
          inherit pkgs lib;
          treefmtNix = treefmt-nix;
        };
        config = import ./lib/build-config.nix {inherit pkgs lib;};
      in
        (bl.mkWorkspace (config.load {root = ./.;})).packages.bloomery;
    };
  in
    (mkSelfFlake {
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

        checks = eachSystem (system: let
          pkgs = nixpkgs.legacyPackages.${system};
          bloomery = bloomeryFor system;
          rootWorkspace = perSystemWorkspace.${system};
          docsPackage = rootWorkspace.packages."bloomery-docs";
          testFlakeChecks = import ./nix/test-flake-checks.nix {
            inherit lib pkgs nixpkgs flake-parts system;
            bloomery = self;
          };
          rootChecks = import ./nix/checks {
            inherit lib pkgs bloomery docsPackage;
            root = ./.;
            workspace = rootWorkspace;
            treefmt = rootWorkspace.formatterConfig;
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
