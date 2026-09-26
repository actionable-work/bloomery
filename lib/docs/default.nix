{ pkgs, lib ? pkgs.lib, cratesIoIndex ? null }:

let
  mkWorkspace = import ../mk-workspace.nix { inherit pkgs lib cratesIoIndex; };
  workspace = mkWorkspace {
    root = ../../docs;
    unifyFeatures = true;
    inherit cratesIoIndex;
    profile = {
      optLevel = "3";
      lto = "thin";
      codegenUnits = 1;
    };
  };
in
  workspace.packages."bloomery-docs"

