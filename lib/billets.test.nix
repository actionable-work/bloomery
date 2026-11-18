{
  lib,
  mkFlake,
  pkgs,
}: let
  systems = ["x86_64-linux" "aarch64-linux"];
  nixpkgs = {
    inherit lib;
    legacyPackages = lib.genAttrs systems (system: pkgs // {testSystem = system;});
  };
  workspace = args:
    mkFlake ({
        inherit nixpkgs;
        systems = ["x86_64-linux"];
        root = ../tests/basic-workspace;
      }
      // args);
  composed = workspace {
    inherit systems;
    root = ../tests/composition-fixture;
    billets = {
      pkgs,
      packages,
    }: {
      service = {
        inherit (pkgs) testSystem;
        package = packages."host-app";
        validationCheck = pkgs.hello;
      };
      bundle = {
        package = packages."complete:complete-pkg";
        validationCheck = pkgs.cowsay;
      };
    };
    extraOutputs = {eachSystem, ...}: {
      checks = eachSystem (_: {custom = pkgs.hello;});
    };
  };
  system = "x86_64-linux";
  selecting = workspace {billets = _: {service.validationCheck = pkgs.hello;};};
  plain = workspace {};
  legacy = workspace {
    extraOutputs = _: {billets.${system}.custom = "custom-output";};
  };
in {
  "billet:output-contract" = assert builtins.attrNames composed.billets == ["aarch64-linux" "x86_64-linux"];
  assert lib.all (system: let
    entries = composed.billets.${system};
    packages = composed.packages.${system};
    checks = composed.checks.${system};
  in
    builtins.attrNames entries
    == ["bundle" "complete:worker" "service"]
    && entries.service.testSystem == system
    && entries.service.package.drvPath == packages."host-app".drvPath
    && entries.bundle.package.drvPath == packages."complete:complete-pkg".drvPath
    && checks."billet:service:validate".drvPath == pkgs.hello.drvPath
    && checks."billet:bundle:validate".drvPath == pkgs.cowsay.drvPath
    && builtins.hasAttr "complete:complete:check" checks
    && builtins.hasAttr "custom" checks)
  systems;
  assert builtins.hasAttr "bin-calc:test" selecting.checks.${system};
  assert !(plain ? billets);
  assert legacy.billets.${system}.custom == "custom-output";
  assert builtins.attrNames plain.checks.${system} == builtins.attrNames legacy.checks.${system};
    pkgs.runCommand "bloomery-billet-output-contract" {
      passthru.bloomery = [
        "NIXLIB-FLAKE-BILLETS-001"
        "NIXLIB-FLAKE-BILLETS-002"
        "NIXLIB-FLAKE-BILLETS-003"
        "NIXLIB-FLAKE-BILLETS-004"
        "NIXLIB-FLAKE-BILLETS-009"
      ];
    } ''touch "$out"'';
}
