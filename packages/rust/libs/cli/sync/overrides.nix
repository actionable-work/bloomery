{
  lib,
  # Overridable so identity tests can exercise the actual filtering against
  # paired enclosing repository snapshots.
  sourceRoot ? ./../../../../..,
  ...
}: let
  # Only the Nix API files inspected by the migration tests enter the support
  # tree. lib/lock/default.nix is intentionally absent.
  migrationSources = lib.fileset.toSource {
    root = sourceRoot;
    fileset = lib.fileset.unions [
      (sourceRoot + "/lib/default.nix")
      (sourceRoot + "/lib/mk-workspace.nix")
      (sourceRoot + "/lib/mk-flake.nix")
      (sourceRoot + "/lib/modules/flake-module.nix")
      (sourceRoot + "/lib/workspace/lock-check.nix")
    ];
  };
in {
  test = {
    env.BLOOMERY_SOURCE_ROOT = "${migrationSources}";
  };
}
