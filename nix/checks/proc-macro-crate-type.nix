{
  pkgs,
  workspace,
}: let
  procMacro = workspace.crates."bloomery-test-macros-0.1.0";
in
  pkgs.runCommand "bloomery-proc-macro-crate-type" {
    passthru.bloomery = [
      "NIXLIB-GRAPH-DERIVATIONS-022"
    ];
  } ''
    echo "Validating the proc-macro fixture crate type..."
    if ls ${procMacro}/lib/libbloomery_test_macros*.so >/dev/null 2>&1 \
      || ls ${procMacro}/lib/libbloomery_test_macros*.dylib >/dev/null 2>&1; then
      :
    else
      echo "missing proc-macro dynamic library output in ${procMacro}/lib"
      ls ${procMacro}/lib || true
      exit 1
    fi
    mkdir "$out"
    echo "OK" > "$out/success"
  ''
