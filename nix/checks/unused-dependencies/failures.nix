# Each dependency section reports only the entries its units never use.
{expectLog, ...}: {
  "core:unused-dependencies-normal" = expectLog {
    name = "unused-dependencies-normal";
    passthru.bloomery = ["NIXLIB-GRAPH-UNUSED-002" "NIXLIB-GRAPH-UNUSED-003"];
    script = ''
      reported "dependencies entry 'dep-unused' (extern \`dep_unused\`)"
      reported "dependencies entry 'dep-test-only' (extern \`dep_test_only\`)"
      absent "entry 'dep-lib'"
      absent "entry 'dep-bin'"
    '';
  };
  "core:unused-dependencies-dev" = expectLog {
    name = "unused-dependencies-dev";
    passthru.bloomery = ["NIXLIB-GRAPH-UNUSED-004"];
    script = ''
      reported "dev-dependencies entry 'dep-dev-unused'"
      absent "entry 'dep-test'"
      absent "entry 'dep-example'"
    '';
  };
  "core:unused-dependencies-build" = expectLog {
    name = "unused-dependencies-build";
    passthru.bloomery = ["NIXLIB-GRAPH-UNUSED-005"];
    script = ''
      reported "build-dependencies entry 'dep-build-unused'"
      absent "entry 'dep-build'"
    '';
  };
}
