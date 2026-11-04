{
  pkgs,
  lib ? pkgs.lib,
}: let
  override = import ../packages/rust/libs/cli/init/overrides.nix {inherit lib;};
  templates = override.env.BLOOMERY_TEMPLATES_DIR;
  config = builtins.fromTOML (builtins.readFile ../.bloomery/config.toml);
  scannerPaths = lib.concatLists (map (scanner: scanner.paths or []) (lib.attrValues config.scanners));
  templateFixtures = {
    axum = {
      project = "template_axum";
      root = ../tests/template-axum;
    };
    basic = {
      project = "template_basic";
      root = ../tests/template-basic;
    };
    topcoat = {
      project = "template_topcoat";
      root = ../tests/template-topcoat;
    };
  };
  files = root: let
    walk = directory: prefix: let
      entries = builtins.readDir directory;
    in
      lib.concatLists (map (
        name: let
          path = directory + "/${name}";
          relative =
            if prefix == ""
            then name
            else "${prefix}/${name}";
          kind = entries.${name};
        in
          if kind == "directory"
          then walk path relative
          else if kind == "regular"
          then [
            {
              inherit relative;
              content = builtins.readFile path;
            }
          ]
          else []
      ) (builtins.attrNames entries));
  in
    walk root "";
  sortFiles = entries: lib.sort (left: right: left.relative < right.relative) entries;
  generatedLockfiles = ["Cargo.lock" "bloomery.lock" "flake.lock"];
  templateMatchesFixture = name: fixture: let
    rendered = map (
      entry: {
        relative = lib.replaceStrings ["{{project}}"] [fixture.project] entry.relative;
        content = lib.replaceStrings ["{{project}}"] [fixture.project] entry.content;
      }
    ) (files (templates + "/${name}"));
    fixtureFiles = builtins.filter (entry: !(builtins.elem entry.relative generatedLockfiles)) (files fixture.root);
  in
    sortFiles rendered == sortFiles fixtureFiles;
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

  testRenderedTemplateFixturesMatchTheirSources = {
    expr = builtins.all (name: templateMatchesFixture name templateFixtures.${name}) (builtins.attrNames templateFixtures);
    expected = true;
    passthru.bloomery = ["CLI-INIT-TEMPLATES-012"];
  };
}
