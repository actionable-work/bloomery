{
  pkgs,
  workspace,
}: let
  itoa = import ../../lib/test-package.nix workspace.lock "itoa";
  archive = "${itoa.id}.crate";
  source = workspace.crates.${itoa.id}.src;
in
  assert source.name == archive;
  assert source.url == "https://static.crates.io/crates/itoa/${archive}";
    pkgs.runCommand "validate-basic-workspace-registry-source" {
      passthru.bloomery = ["NIXLIB-GRAPH-RESOLUTION-013" "REPOSITORY-LAYOUT-STRUCTURE-013"];
    } ''
      touch "$out"
    ''
