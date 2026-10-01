{
  lib,
  pkgs,
  nixpkgs,
  flake-parts,
  bloomery,
  system,
}: let
  testDirs = builtins.attrNames (lib.filterAttrs (
    name: type: type == "directory" && builtins.pathExists (../tests + "/${name}/flake.nix")
  ) (builtins.readDir ../tests));

  testFlakes = lib.genAttrs testDirs (name:
    (import (../tests + "/${name}/flake.nix")).outputs {
      # flake-parts requires the standard self output argument when called directly.
      self = {
        inputs = {
          inherit nixpkgs;
        };
      };
      inherit nixpkgs flake-parts;
      inherit bloomery;
    });
in
  lib.mapAttrs' (name: testFlake: let
    checks = testFlake.checks.${system} or {};
    checkNames = builtins.attrNames checks;
    checkInputs =
      lib.concatMapStringsSep "\n" (
        checkName: "test -e ${checks.${checkName}}"
      )
      checkNames;
  in
    assert checkNames != [];
      lib.nameValuePair "tests:${name}" (pkgs.runCommand "bloomery-${name}-checks" {} ''
        echo "All checks for ${name} passed."
        ${checkInputs}
        mkdir "$out"
        echo "passed" > "$out/success"
      ''))
  testFlakes
