{
  inputs,
  self,
  ...
}: let
  inherit (inputs) nixpkgs treefmt-nix flake-parts;
  inherit (nixpkgs) lib;
  systems = import ./systems.nix;
  eachSystem = lib.genAttrs systems;

  bloomeryFor = system: let
    pkgs = nixpkgs.legacyPackages.${system};
  in
    import ../lib {
      inherit pkgs;
      inherit (pkgs) lib;
    };

  mkFlake = import ../lib/mk-flake.nix {
    bloomeryLib = {
      pkgs,
      lib ? pkgs.lib,
    }:
      import ../lib {inherit pkgs lib;};
  };

  flakeModules = {
    default = import ../lib/modules/flake-module.nix;
  };
  flakeModule = flakeModules.default;

  mkLib =
    {
      __functor = _self: pkgs:
        import ../lib {
          inherit pkgs;
          inherit (pkgs) lib;
        };
    }
    // (eachSystem bloomeryFor);
in {
  systems = systems;

  flake = {
    inherit mkFlake flakeModules flakeModule mkLib;

    lib =
      (eachSystem bloomeryFor)
      // {
        inherit mkFlake flakeModules flakeModule;
        parseLock = import ../lib/workspace/parse-lock.nix {inherit lib;};
      };
  };

  perSystem = {
    system,
    pkgs,
    ...
  }: let
    bloomery = bloomeryFor system;
    rootWorkspace = bloomery.mkWorkspace {
      root = ../.;
      createLibPackages = true;
      createDevPackages = true;
      profile = {
        optLevel = "3";
        lto = "thin";
        codegenUnits = 1;
      };
    };
    docsPackage = rootWorkspace.packages."bloomery-docs";
    docsDevPackage = rootWorkspace.packages."bloomery-docs:dev";
    treefmtModule = treefmt-nix.lib.evalModule pkgs ../nix/lib/treefmt-config.nix;
    testFlakeChecks = import ./test-flake-checks.nix {
      inherit lib pkgs nixpkgs flake-parts system;
      bloomery = self;
    };
    rootChecks =
      lib.mapAttrs'
      (name: drv: lib.nameValuePair "core:${name}" drv)
      rootWorkspace.checks;
    docsAssetsCheck = pkgs.runCommand "validate-docs-assets" {} ''
      echo "Validating docs assets in ${docsPackage}..."
      test -d "${docsPackage}/bin/assets" || { echo "Missing bin/assets"; exit 1; }
      test -f "${docsPackage}/bin/assets/bloomery.css" || { echo "Missing bloomery.css"; exit 1; }
      test -f "${docsPackage}/bin/assets/bloomery-forge.svg" || { echo "Missing bloomery-forge.svg"; exit 1; }
      test -f "${docsPackage}/bin/assets/favicon.svg" || { echo "Missing favicon.svg"; exit 1; }
      test -f "${docsPackage}/bin/assets/manifest.toml" || { echo "Missing generated manifest.toml"; exit 1; }
      test -d "${docsPackage}/share/bloomery-docs/assets" || { echo "Missing share assets"; exit 1; }
      mkdir $out
      echo "OK" > $out/success
    '';
  in {
    apps =
      rootWorkspace.apps
      // {
        docs = rootWorkspace.apps."bloomery-docs";
        "docs:dev" = rootWorkspace.apps."bloomery-docs:dev";
        default = rootWorkspace.apps.lock;
      };

    packages =
      rootWorkspace.packages
      // {
        docs = docsPackage;
        "docs:dev" = docsDevPackage;
        lock = bloomery.lock.lockScript;
        default = docsPackage;
      };

    formatter = treefmtModule.config.build.wrapper;

    checks =
      {
        "core:unit-tests" = bloomery.tests.check;
        "core:treefmt-check" = import ../nix/checks/treefmt-check.nix {
          inherit pkgs;
          treefmt = treefmtModule.config;
          root = ../.;
        };
        "core:validate-docs-assets" = docsAssetsCheck;
      }
      // rootChecks
      // testFlakeChecks;

    devShells.default = rootWorkspace.devShell;
  };
}
