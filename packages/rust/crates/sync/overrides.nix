{pkgs, ...}: let
  sourceRoot = ./../../../..;
  migrationSources = pkgs.runCommand "bloomery-sync-migration-test-sources" {} ''
    mkdir -p "$out"
    ln -s ${sourceRoot}/lib "$out/lib"
  '';
in {
  # Keep the Nix API sources available to the isolated migration tests.
  nativeBuildInputs = [migrationSources];
  env.BLOOMERY_SOURCE_ROOT = "${migrationSources}";
}
