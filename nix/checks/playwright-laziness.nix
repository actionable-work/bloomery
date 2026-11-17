{
  pkgs,
  lib,
}: let
  failures = lib.runTests (import ../../lib/playwright/laziness.test.nix {inherit pkgs lib;});
in
  assert failures == [];
    pkgs.runCommand "playwright-laziness" {
      passthru.bloomery = ["NIXLIB-PLAYWRIGHT-RUNNER-009"];
    } ''
      mkdir "$out"
    ''
