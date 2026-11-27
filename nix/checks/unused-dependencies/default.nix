# Compiler-backed unused dependency checks over tests/unused-deps-workspace,
# whose `app` member declares unused entries in every dependency section.
{
  pkgs,
  lib,
}: let
  mkWorkspace = import ../../../lib/mk-workspace.nix {inherit pkgs lib;};
  root = ../../../tests/unused-deps-workspace;
  appDir = root + "/crates/app";
  checks = {
    enable = true;
    unusedDependencies = {
      compiler = true;
      exceptions = ["dep-ignored"];
    };
  };
  workspace = mkWorkspace {inherit root checks;};
  # `app` without its examples, which no longer satisfy dev-dependencies.
  selectedWorkspace = mkWorkspace {
    inherit root checks;
    overrides.app.fileset = lib.fileset.unions (map (path: appDir + path) ["/Cargo.toml" "/build.rs" "/src" "/tests"]);
  };
  # `app` declares unused dependencies on purpose, so its checks must fail.
  appFailure = pkgs.testers.testBuildFailure workspace.checks."app:unused-dependencies";
  selectedFailure = pkgs.testers.testBuildFailure selectedWorkspace.checks."app:unused-dependencies";
  # Run assertions against a failed `app:unused-dependencies` build log.
  expectLog = {
    name,
    passthru,
    script,
    failure ? appFailure,
  }:
    pkgs.runCommand "bloomery-${name}" {inherit passthru;} ''
      log=${failure}/testBuildFailure.log
      reported() { grep -F "$1" "$log" > /dev/null || { echo "missing from log: $1" >&2; exit 1; }; }
      absent() { if grep -F "$1" "$log" > /dev/null; then echo "unexpected in log: $1" >&2; exit 1; fi; }
      ${script}
      touch "$out"
    '';
  shared = {inherit workspace appFailure selectedFailure expectLog;};
in
  import ./failures.nix shared
  // import ./report.nix shared
  // import ./targets.nix shared
  // import ./skipped.nix shared
  // {
    "core:unused-dependencies-units" = import ./units.nix {inherit pkgs lib;};
    "core:unused-dependencies-loading" = import ./loading.nix {inherit pkgs lib;};
  }
