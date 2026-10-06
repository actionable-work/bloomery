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
        flakeParts = flake-parts;
      };

    bloomeryFor = system:
      bloomeryLib {pkgs = nixpkgs.legacyPackages.${system};};

    # The Bloomery API injectable into sub-flake inputs by default, so nested
    # `bloomery.mkFlake` calls inherit it without the caller threading `self`.
    bloomeryApi = {inherit mkFlake;};

    # The repository builds its own workspace without a pinned CLI so local
    # iteration is never shadowed by a prebuilt binary.
    mkSelfFlake = import ./lib/mk-flake.nix {
      inherit bloomeryLib;
      defaultInputs = {bloomery = bloomeryApi;};
    };

    # Consumers always receive the Bloomery CLI in the default development
    # shell, built from this flake's workspace with their nixpkgs.
    mkFlake = import ./lib/mk-flake.nix {
      inherit bloomeryLib;
      defaultInputs = {bloomery = bloomeryApi;};
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
      self = {
        inputs = {
          inherit nixpkgs treefmt-nix flake-parts;
        };
      };
      extraOutputs = {
        eachSystem,
        perSystemWorkspace,
      }: {
        checks = eachSystem (
          system:
            import ./nix/root-checks.nix {
              inherit lib nixpkgs self bloomeryFor perSystemWorkspace system;
            }
        );
        apps = eachSystem (
          system: let
            pkgs = nixpkgs.legacyPackages.${system};
            bloomeryCli = perSystemWorkspace.${system}.packages.bloomery;
            bench = pkgs.writeShellApplication {
              name = "bloomery-bench";
              runtimeInputs = [
                pkgs.bash
                pkgs.cargo
                pkgs.git
                pkgs.hyperfine
                pkgs.python3
                pkgs.rustc
              ];
              text = ''
                export BLOOMERY_BIN="${bloomeryCli}/bin/bloomery"
                exec ${pkgs.bash}/bin/bash ${./benchmarks/run.sh} "$@"
              '';
            };
          in
            perSystemWorkspace.${system}.apps
            // {
              bench = {
                type = "app";
                program = "${bench}/bin/bloomery-bench";
              };
            }
        );
      };
    })
    // {
      inherit mkFlake;
    };
}
