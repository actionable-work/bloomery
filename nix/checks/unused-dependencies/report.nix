# Exceptions stay silent, source allows cannot hide findings, build scripts see
# Cargo's environment, and failures explain the repair.
{
  workspace,
  appFailure,
  expectLog,
  ...
}: {
  "core:unused-dependencies-exceptions" = expectLog {
    name = "unused-dependencies-exceptions";
    passthru.bloomery = ["NIXLIB-GRAPH-UNUSED-006"];
    script = ''
      absent "dep-ignored"
    '';
  };
  "core:unused-dependencies-allow" = expectLog {
    name = "unused-dependencies-allow";
    passthru.bloomery = ["NIXLIB-GRAPH-UNUSED-011"];
    script = ''
      # The `app` library allows `unused_crate_dependencies` at its root.
      reported "dependencies entry 'dep-unused'"
      absent "Could not compile"
    '';
  };
  "core:unused-dependencies-build-environment" = expectLog {
    name = "unused-dependencies-build-environment";
    passthru.bloomery = ["NIXLIB-GRAPH-UNUSED-013"];
    script = ''
      # The build script needs CARGO_CFG_* and emits the cfg that gates the
      # library's use of `core-lib`.
      absent "Build script build.rs of app failed"
      absent "entry 'dep-lib'"
    '';
  };
  "core:unused-dependencies-report" = expectLog {
    name = "unused-dependencies-report";
    passthru.bloomery = ["NIXLIB-GRAPH-UNUSED-007"];
    script = ''
      reported "UNUSED DEPENDENCY CHECK FAILED for app"
      reported "checks.unusedDependencies.exceptions"
      reported "add \`use <extern> as _;\` to a target that needs it only for linking"
      test "$(cat ${appFailure}/testBuildFailure.exit)" != 0
      test -f ${workspace.checks."dep-lib:unused-dependencies"}/unused-dependencies.txt
    '';
  };
}
