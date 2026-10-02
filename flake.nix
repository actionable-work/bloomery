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
    bloomeryPackageForSystem = system: self.packages.${system}.bloomery;

    bloomeryFor = system: let
      pkgs = nixpkgs.legacyPackages.${system};
    in
      import ./lib {
        inherit pkgs bloomeryPackageForSystem;
        inherit (pkgs) lib;
      };

    mkFlake = import ./lib/mk-flake.nix {
      bloomeryLib = {
        pkgs,
        lib ? pkgs.lib,
      }:
        import ./lib {inherit pkgs lib bloomeryPackageForSystem;};
    };

    flakeModules = {
      default = args:
        import ./lib/modules/flake-module.nix (args // {inherit bloomeryPackageForSystem;});
    };
    flakeModule = flakeModules.default;

    eachSystem = lib.genAttrs systems;
    mkLib =
      {
        __functor = _self: pkgs:
          import ./lib {
            inherit pkgs bloomeryPackageForSystem;
            inherit (pkgs) lib;
          };
      }
      // (eachSystem bloomeryFor);
  in
    (mkFlake {
      inherit nixpkgs systems;
      root = ./.;
      createLibPackages = false;
      createDevPackages = true;
      profile = {
        optLevel = "3";
        lto = "fat";
        codegenUnits = 1;
      };
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
      inherit mkFlake flakeModules flakeModule mkLib;

      lib =
        (eachSystem bloomeryFor)
        // {
          inherit mkFlake flakeModules flakeModule;
          parseLock = import ./lib/workspace/parse-lock.nix {inherit lib;};
        };
    };
}
