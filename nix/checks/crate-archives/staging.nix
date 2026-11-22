{fixture}: let
  inherit (fixture) b c d main check;
in
  check "staging" {
    passthru.bloomery = [
      "NIXLIB-GRAPH-DERIVATIONS-031"
      "NIXLIB-GRAPH-ARCHIVES-010"
      "NIXLIB-GRAPH-ARCHIVES-012"
      "NIXLIB-GRAPH-ARCHIVES-013"
      "NIXLIB-GRAPH-ARCHIVES-014"
    ];
  } ''
    # Count extractions through a tar shim.
    mkdir shim
    real_tar="$(command -v tar)"
    printf '#!%s\nfor arg in "$@"; do case "$arg" in *.tar.zst) echo "$arg" >> %s ;; esac; done\nexec %s "$@"\n' \
      "$BASH" "$PWD/extractions" "$real_tar" > shim/tar
    chmod +x shim/tar

    echo "Link staging..."
    LINK_EXTERNS=() LINK_FLAGS=()
    PATH="$PWD/shim:$PATH" resolve_deps LINK_EXTERNS LINK_FLAGS "$PWD/link" link ${b} ${c}
    [ -s extractions ] || fail "staging extracted no archives through the tar shim"
    [ "$(sort extractions | uniq -d)" = "" ] || fail "an archive was extracted more than once"
    printf '%s\n' "''${LINK_EXTERNS[@]}" | grep -qE "^b=$PWD/link/libb-.*\.rlib$" || fail "link staging does not bind b's rlib"
    mkdir full
    for dep in ${b} ${c}; do
      while read -r entry; do
        for archive in "$entry/meta.tar.zst" "$entry/lib.tar.zst"; do
          if [ -f "$archive" ]; then tar --zstd -xf "$archive" -C full; fi
        done
      done < "$dep/nix-support/deps-closure"
    done
    diff -r link full || fail "deduplicated staging differs from full extraction"

    echo "Meta staging..."
    META_EXTERNS=() META_FLAGS=()
    resolve_deps META_EXTERNS META_FLAGS "$PWD/meta" meta ${b} ${c}
    [ -z "$(find meta -name '*.rlib')" ] || fail "meta staging extracted an rlib"
    printf '%s\n' "''${META_EXTERNS[@]}" | grep -qE "^c=$PWD/meta/libc-.*\.rmeta$" || fail "meta staging does not bind c's rmeta"

    echo "Linking against crates compiled from metadata..."
    D_EXTERNS=() D_FLAGS=()
    resolve_deps D_EXTERNS D_FLAGS "$PWD/bin" link ${d}
    rustc --edition=2021 ${main} -L dependency="$PWD/bin" "''${D_EXTERNS[@]}" -Clink-arg=-fuse-ld=lld -o main
    [ "$(./main)" = 45 ] || fail "linked binary printed $(./main)"
  ''
