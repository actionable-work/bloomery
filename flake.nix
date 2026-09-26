{
  description = "bloomery: Pure Nix Rust builder without Cargo, pulling dependencies directly from Cargo.lock";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs, ... }:
    let
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
        parseLock = import ./lib/workspace/parse-lock.nix { inherit (nixpkgs) lib; };
      };

      # Per-system runnable applications
      apps = eachSystem (system:
        let
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
            program = "${bloomery.lock.lockScript}/bin/bloomery-lock";
          };
        in {
          docs = docsApp;
          lock = lockApp;
          default = lockApp;
        }
      );

      # Per-system outputs
      packages = eachSystem (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          bloomery = import ./lib {
            inherit pkgs;
            inherit (pkgs) lib;
          };
        in {
          docs = bloomery.docs;
          lock = bloomery.lock.lockScript;
          bloomery-lock = bloomery.lock.lockScript;
          default = bloomery.docs;
        }
      );

      checks = eachSystem (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          unitTests = import ./tests/unit-tests.nix {
            inherit pkgs;
            inherit (pkgs) lib;
          };
        in {
          unit-tests = unitTests.check;
        }
      );

      # Per-system builder library
      mkLib = eachSystem (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in import ./lib {
          inherit pkgs;
          inherit (pkgs) lib;
        }
      );

      devShells = eachSystem (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in {
          default = pkgs.mkShell {
            packages = [
              pkgs.rustc
              pkgs.clippy
              pkgs.cargo
              pkgs.jq
            ];
          };
        }
      );
    };
}
