{
  lib,
  mkFlake,
  pkgs,
}: let
  systems = ["x86_64-linux" "aarch64-linux"];
  system = "x86_64-linux";
  nixpkgs = {
    inherit lib;
    legacyPackages = lib.genAttrs systems (_: pkgs);
  };
  flake = mkFlake {
    inherit nixpkgs systems;
    root = ../tests/basic-workspace;
    billets = {packages, ...}: {
      service = {
        package = packages."bin-calc";
        validationCheck = pkgs.hello;
      };
    };
    extraOutputs = {eachSystem, ...}: {
      checks = eachSystem (_: {
        custom = pkgs.cowsay;
        broken = throw "broken check";
        "bloomery:check" = pkgs.hello;
      });
    };
  };
  checks = flake.checks.${system};
  # The CLI reads records as text, so references do not matter here.
  readRecord = record: field: builtins.fromJSON (builtins.unsafeDiscardStringContext record.${field});
  catalog = readRecord flake.bloomeryCatalog.${system} "catalog";
  plan = readRecord flake.bloomeryPlan.${system} "plan";
  formatter = flake.formatter.${system};
  plainReferences = record: field:
    lib.all (entry: entry == {path = true;}) (builtins.attrValues (builtins.getContext record.${field}));
  records =
    lib.concatMap (system: [
      (flake.bloomeryCatalog.${system} // {field = "catalog";})
      (flake.bloomeryPlan.${system} // {field = "plan";})
    ])
    systems;
in {
  "check-catalog:records" =
    # Catalog facts mirror the final outputs.
    assert catalog.checks == builtins.attrNames (builtins.removeAttrs checks ["bloomery:check"]);
    assert catalog.legacy;
    assert catalog.formatCheck
    == {
      success = true;
      drvPath = checks."core:treefmt-check".drvPath;
    };
    assert catalog.formatter
    == {
      drvPath = formatter.drvPath;
      program = lib.getExe formatter;
    };
    assert catalog.healthSources == flake.healthSources.${system};
    # The plan records every other check, isolating evaluation failures.
    assert plan.legacy;
    assert builtins.attrNames plan.plan == builtins.attrNames (builtins.removeAttrs checks ["bloomery:check" "core:treefmt-check"]);
    assert plan.plan.custom
    == {
      success = true;
      drvPath = pkgs.cowsay.drvPath;
    };
    assert plan.plan.broken == {success = false;};
    # Composed, elevated, and Billet checks are cataloged.
    assert lib.all (name: builtins.elem name catalog.checks && plan.plan ? ${name}) ["custom" "billet:service:validate" "bin-calc:test"];
    # Records exist for every system, are never built, and only reference
    # recorded `.drv` files.
    assert lib.all (record: record.builder == "/bin/false") records;
    assert map (record: record.system) records == ["x86_64-linux" "x86_64-linux" "aarch64-linux" "aarch64-linux"];
    assert lib.all (record: plainReferences record record.field) records;
      pkgs.runCommand "bloomery-check-catalog-records" {
        passthru.bloomery = [
          "NIXLIB-FLAKE-OUTPUTS-031"
          "NIXLIB-FLAKE-OUTPUTS-032"
          "NIXLIB-FLAKE-OUTPUTS-033"
          "NIXLIB-FLAKE-OUTPUTS-034"
          "NIXLIB-FLAKE-OUTPUTS-035"
        ];
      } ''
        touch "$out"
      '';
}
