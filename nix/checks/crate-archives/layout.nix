{fixture}: let
  inherit (fixture) a aMetadata m check;
in
  check "layout" {
    passthru.bloomery = [
      "NIXLIB-GRAPH-DERIVATIONS-027"
      "NIXLIB-GRAPH-ARCHIVES-001"
      "NIXLIB-GRAPH-ARCHIVES-002"
      "NIXLIB-GRAPH-ARCHIVES-003"
    ];
  } ''
    echo "Archive layout..."
    tar --zstd -tf ${a}/meta.tar.zst | grep -qE '^\./liba-.*\.rmeta$' || fail "a meta archive lacks its rmeta"
    if tar --zstd -tf ${a}/meta.tar.zst | grep -q '\.rlib$'; then fail "a meta archive holds an rlib"; fi
    tar --zstd -tf ${a}/lib.tar.zst | grep -qE '^\./liba-.*\.rlib$' || fail "a lib archive lacks its rlib"
    tar --zstd -tf ${m}/meta.tar.zst | grep -qE '^\./libm-.*\.so$' || fail "m meta archive lacks its proc-macro library"
    test ! -e ${m}/lib.tar.zst || fail "proc macro publishes a lib archive"
    test ! -e ${aMetadata}/lib.tar.zst || fail "metadata-only variant publishes a lib archive"
    test ! -e ${a}/lib || fail "crate output has an uncompressed library directory"
  ''
