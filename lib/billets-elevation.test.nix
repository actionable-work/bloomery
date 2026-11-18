{
  lib,
  mkFlake,
  pkgs,
}: let
  inherit (pkgs.stdenv.hostPlatform) system;
  flake = mkFlake {
    nixpkgs = {
      inherit lib;
      legacyPackages.${system} = pkgs;
    };
    systems = [system];
    root = ../tests/composition-fixture;
  };
  entries = flake.billets.${system};
  checks = flake.checks.${system};
in
  assert builtins.attrNames entries == ["complete:worker"];
  assert entries."complete:worker".role == "complete-worker";
  assert builtins.hasAttr "complete:billet:worker:validate" checks;
  assert !(lib.any (lib.hasPrefix "billet:") (builtins.attrNames checks));
    pkgs.runCommand "bloomery-billet-elevation" {
      passthru.bloomery = [
        "NIXLIB-FLAKE-COMPOSITION-021"
        "NIXLIB-FLAKE-COMPOSITION-022"
      ];
    } ''touch "$out"''
