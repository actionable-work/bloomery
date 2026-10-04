{
  lib,
  # Overridable so identity tests can exercise the actual filtering against
  # paired enclosing repository snapshots.
  sourceRoot ? ./../../../../..,
  ...
}: let
  # Only the files inspected by the layout tests enter the support tree.
  layoutTestSources = lib.fileset.toSource {
    root = sourceRoot;
    fileset = lib.fileset.unions [
      (sourceRoot + "/Cargo.toml")
      (sourceRoot + "/packages/rust/bins/cli/src/main.rs")
      (sourceRoot + "/packages/rust/bins/docs/src/main.rs")
    ];
  };
in {
  test = {
    env.BLOOMERY_REPOSITORY_ROOT = "${layoutTestSources}";
  };
}
