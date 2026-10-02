{
  pkgs,
  lib ? pkgs.lib,
}: let
  mkWorkspace = import ../mk-workspace.nix {inherit pkgs lib;};
  workspace = mkWorkspace {
    root = ../..;
    profile = {
      optLevel = "3";
      lto = "thin";
      codegenUnits = 1;
    };
  };
  releasePkg = workspace.packages."bloomery-docs";
in
  releasePkg
