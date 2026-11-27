# Targets outside a custom source selection leave their sections unchecked.
{
  expectLog,
  selectedFailure,
  ...
}: {
  "core:unused-dependencies-skipped" = expectLog {
    name = "unused-dependencies-skipped";
    failure = selectedFailure;
    passthru.bloomery = ["NIXLIB-GRAPH-UNUSED-014"];
    script = ''
      reported "Not checking dev declarations: skipped example examples/plugin.rs (outside the source selection)"
      absent "dev-dependencies entry"
      reported "dependencies entry 'dep-unused'"
    '';
  };
}
