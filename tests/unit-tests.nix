{
  pkgs,
  lib ? pkgs.lib,
}: let
  bloomery = import ../lib {inherit pkgs lib;};
in
  bloomery.tests
