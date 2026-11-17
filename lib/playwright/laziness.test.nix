{
  pkgs,
  lib,
}: let
  # Accessing either package outside a browser-test derivation is an error.
  guardedPkgs =
    pkgs
    // {
      playwright-test = throw "Playwright loaded outside a browser check";
      playwright-driver = throw "Chromium loaded outside a browser check";
    };
  config = import ../build-config.nix {
    pkgs = guardedPkgs;
    inherit lib;
  };
  workspace = import ../mk-workspace.nix {
    pkgs = guardedPkgs;
    inherit lib;
  } (config.load {root = ../..;});
in {
  testBrowserToolsAreLazy = {
    expr = builtins.deepSeq [
      workspace.packages.bloomery.drvPath
      workspace.packages.bloomery-docs.drvPath
      workspace.devShell.drvPath
    ] (builtins.isFunction workspace.playwrightTest);
    expected = true;
  };
}
