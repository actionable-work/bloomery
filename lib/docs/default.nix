{ pkgs, lib ? pkgs.lib }:

let
  mkWorkspace = import ../mk-workspace.nix { inherit pkgs lib; };
  workspace = mkWorkspace {
    root = ../../docs;
    profile = {
      optLevel = "3";
      lto = "thin";
      codegenUnits = 1;
    };
  };
in
  workspace.packages."bloomery-docs"

