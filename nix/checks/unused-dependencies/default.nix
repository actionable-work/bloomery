# Compiler-backed unused dependency checks over tests/unused-deps-workspace,
# whose `app` member declares unused entries in every dependency section.
{
  pkgs,
  lib,
}: let
  mkWorkspace = import ../../../lib/mk-workspace.nix {inherit pkgs lib;};
  workspace = mkWorkspace {
    root = ../../../tests/unused-deps-workspace;
    checks = {
      enable = true;
      unusedDependencies = {
        compiler = true;
        exceptions = ["dep-ignored"];
      };
    };
  };
  # `app` declares unused dependencies on purpose, so its check must fail.
  appFailure = pkgs.testers.testBuildFailure workspace.checks."app:unused-dependencies";
  # Run assertions against the failed `app:unused-dependencies` build log.
  expectLog = {
    name,
    passthru,
    script,
  }:
    pkgs.runCommand "bloomery-${name}" {inherit passthru;} ''
      log=${appFailure}/testBuildFailure.log
      reported() { grep -F "$1" "$log" > /dev/null || { echo "missing from log: $1" >&2; exit 1; }; }
      absent() { if grep -F "$1" "$log" > /dev/null; then echo "unexpected in log: $1" >&2; exit 1; fi; }
      ${script}
      touch "$out"
    '';
  shared = {inherit workspace appFailure expectLog;};
in
  import ./failures.nix shared
  // import ./report.nix shared
  // {"core:unused-dependencies-units" = import ./units.nix {inherit pkgs lib;};}
