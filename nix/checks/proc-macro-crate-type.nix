{
  pkgs,
  workspace,
}: let
  procMacro = workspace.crates."bloomery-test-macros-0.1.0";
in
  pkgs.runCommand "bloomery-proc-macro-crate-type" {
    nativeBuildInputs = [pkgs.zstd];
    passthru.bloomery = [
      "NIXLIB-GRAPH-DERIVATIONS-022"
    ];
  } ''
    echo "Validating the proc-macro fixture crate type..."
    if ! tar --zstd -tf ${procMacro}/lib.tar.zst | grep -E 'libbloomery_test_macros.*\.(so|dylib)$'; then
      echo "missing proc-macro dynamic library in ${procMacro}/lib.tar.zst" >&2
      exit 1
    fi
    mkdir "$out"
    echo "OK" > "$out/success"
  ''
