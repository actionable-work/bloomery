{
  description = "bloomery: Pure Nix Rust builder without Cargo, pulling dependencies directly from Cargo.lock";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    treefmt-nix,
    ...
  }: let
    systems = [
      "x86_64-linux"
      "aarch64-linux"
      "x86_64-darwin"
      "aarch64-darwin"
    ];
    eachSystem = nixpkgs.lib.genAttrs systems;
  in {
    # System-independent library functions
    lib = {
      parseLock = import ./lib/workspace/parse-lock.nix {inherit (nixpkgs) lib;};
    };

    # Per-system runnable applications
    apps = eachSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
        bloomery = import ./lib {
          inherit pkgs;
          inherit (pkgs) lib;
        };
        docsApp = {
          type = "app";
          program = "${bloomery.docs}/bin/bloomery-docs";
        };
        lockApp = {
          type = "app";
          program = "${bloomery.lock.lockScript}/bin/lock";
        };
      in {
        docs = docsApp;
        lock = lockApp;
        default = lockApp;
      }
    );

    # Per-system outputs
    packages = eachSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
        bloomery = import ./lib {
          inherit pkgs;
          inherit (pkgs) lib;
        };
      in {
        docs = bloomery.docs;
        lock = bloomery.lock.lockScript;
        default = bloomery.docs;
      }
    );

    # Formatter wrapper matching actionable-dioxus
    formatter = eachSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
        treefmt = treefmt-nix.lib.evalModule pkgs ./nix/lib/treefmt-config.nix;
      in
        treefmt.config.build.wrapper
    );

    checks = eachSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
        treefmtModule = treefmt-nix.lib.evalModule pkgs ./nix/lib/treefmt-config.nix;
        treefmtCheck = import ./nix/checks/treefmt-check.nix {
          inherit pkgs;
          treefmt = treefmtModule.config;
          root = ./.;
        };

        unitTests = import ./tests/unit-tests.nix {
          inherit pkgs;
          inherit (pkgs) lib;
        };

        # Discover any test workspace sub-flakes dynamically
        testDirs = builtins.attrNames (nixpkgs.lib.filterAttrs
          (name: type: type == "directory" && builtins.pathExists (./tests + "/${name}/flake.nix"))
          (builtins.readDir ./tests));

        # Import and instantiate each test sub-flake and prefix check names
        subFlakeChecks =
          nixpkgs.lib.foldl' (
            acc: name: let
              subFlake = (import (./tests + "/${name}/flake.nix")).outputs {
                self = null;
                inherit nixpkgs;
                bloomery = self;
              };
              checks = subFlake.checks.${system} or {};
              prefixed =
                nixpkgs.lib.mapAttrs'
                (cname: drv: nixpkgs.lib.nameValuePair "tests:${name}:${cname}" drv)
                checks;
            in
              acc // prefixed
          ) {}
          testDirs;
        bloomery = import ./lib {
          inherit pkgs;
          inherit (pkgs) lib;
        };
        rootWorkspace = bloomery.mkWorkspace {
          root = ./.;
        };
        rootChecks =
          nixpkgs.lib.mapAttrs'
          (cname: drv: nixpkgs.lib.nameValuePair "core:${cname}" drv)
          rootWorkspace.checks;
      in
        {
          "core:unit-tests" = unitTests.check;
          "core:treefmt-check" = treefmtCheck;
        }
        // rootChecks
        // subFlakeChecks
    );

    # Per-system builder library
    mkLib = eachSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
      in
        import ./lib {
          inherit pkgs;
          inherit (pkgs) lib;
        }
    );

    devShells = eachSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
      in {
        default = pkgs.mkShell {
          packages = [
            pkgs.rustc
            pkgs.clippy
            pkgs.cargo
            pkgs.jq
            pkgs.nix-fast-build
          ];
        };
      }
    );
  };
}
