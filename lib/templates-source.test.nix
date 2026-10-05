{
  pkgs,
  lib ? pkgs.lib,
}: let
  override = import ../packages/rust/libs/cli/init/overrides.nix {inherit lib;};
  templates = override.env.BLOOMERY_TEMPLATES_DIR;
  config = builtins.fromTOML (builtins.readFile ../.bloomery/config.toml);
  scannerPaths = lib.concatLists (map (scanner: scanner.paths or []) (lib.attrValues config.scanners));
in {
  testBundledTemplateCatalogIsAnExplicitInput = {
    expr =
      builtins.pathExists (templates + "/basic/Cargo.toml")
      && builtins.pathExists (templates + "/axum/Cargo.toml")
      && builtins.pathExists (templates + "/topcoat/Cargo.toml");
    expected = true;
  };

  testTemplatesAreOutsideScannerTargets = {
    expr = builtins.all (path: !(lib.hasPrefix "templates" path)) scannerPaths;
    expected = true;
  };
}
