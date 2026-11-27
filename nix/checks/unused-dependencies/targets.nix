# Every kind of Cargo target in the `app` member is compiled.
{expectLog, ...}: {
  "core:unused-dependencies-library" = expectLog {
    name = "unused-dependencies-library";
    passthru.bloomery = ["NIXLIB-GRAPH-UNUSED-008"];
    script = ''
      reported "Analyzing library src/lib.rs..."
      reported "Analyzing library unit tests src/lib.rs..."
    '';
  };
  "core:unused-dependencies-binary" = expectLog {
    name = "unused-dependencies-binary";
    passthru.bloomery = ["NIXLIB-GRAPH-UNUSED-015"];
    script = ''
      reported "Analyzing binary src/bin/tool.rs..."
      reported "Analyzing binary unit tests src/bin/tool.rs..."
      absent "entry 'dep-bin'"
    '';
  };
  "core:unused-dependencies-integration-test" = expectLog {
    name = "unused-dependencies-integration-test";
    passthru.bloomery = ["NIXLIB-GRAPH-UNUSED-016"];
    script = ''
      reported "Analyzing test tests/it.rs..."
      absent "entry 'dep-test'"
    '';
  };
  "core:unused-dependencies-examples" = expectLog {
    name = "unused-dependencies-examples";
    passthru.bloomery = ["NIXLIB-GRAPH-UNUSED-017"];
    script = ''
      # `plugin` declares crate-type = ["rlib"] and has no `main`.
      reported "Analyzing example examples/plugin.rs..."
      reported "Analyzing example examples/demo.rs..."
      absent "Could not compile"
    '';
  };
}
