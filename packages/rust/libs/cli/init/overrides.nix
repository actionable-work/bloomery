{lib, ...}: let
  sourceRoot = ./../../../../..;
  templates = lib.fileset.toSource {
    root = sourceRoot;
    fileset = sourceRoot + "/templates";
  };
in {
  env.BLOOMERY_TEMPLATES_DIR = "${templates}/templates";
}
