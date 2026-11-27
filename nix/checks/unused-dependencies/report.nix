# Exceptions stay silent, and failures explain the repair.
{
  workspace,
  appFailure,
  expectLog,
}: {
  "core:unused-dependencies-exceptions" = expectLog {
    name = "unused-dependencies-exceptions";
    passthru.bloomery = ["NIXLIB-GRAPH-UNUSED-006"];
    script = ''
      absent "dep-ignored"
    '';
  };
  "core:unused-dependencies-report" = expectLog {
    name = "unused-dependencies-report";
    passthru.bloomery = ["NIXLIB-GRAPH-UNUSED-007"];
    script = ''
      reported "UNUSED DEPENDENCY CHECK FAILED for app"
      reported "checks.unusedDependencies.exceptions"
      test "$(cat ${appFailure}/testBuildFailure.exit)" != 0
      test -f ${workspace.checks."dep-lib:unused-dependencies"}/unused-dependencies.txt
    '';
  };
}
