{
  description = "bloomery: Pure Nix Rust builder without Cargo, pulling dependencies directly from Cargo.lock";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    registry-crates-io = {
      url = "github:rust-lang/crates.io-index";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, registry-crates-io ? null, ... }:
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

      # Per-system runnable applications (Topcoat documentation server)
      apps = eachSystem (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          bloomery = import ./lib {
            inherit pkgs;
            inherit (pkgs) lib;
            cratesIoIndex = registry-crates-io;
          };
          docsApp = {
            type = "app";
            program = "${bloomery.docs}/bin/bloomery-docs";
          };
        in {
          docs = docsApp;
          default = docsApp;
        }
      );

      # Per-system outputs (Topcoat documentation application)
      packages = eachSystem (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          bloomery = import ./lib {
            inherit pkgs;
            inherit (pkgs) lib;
            cratesIoIndex = registry-crates-io;
          };
        in {
          docs = bloomery.docs;
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
          cratesIoIndex = registry-crates-io;
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
            ];
          };
        }
      );
    };
}
