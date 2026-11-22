{fixture}: let
  inherit (fixture) a m d dRebuilt check;
in
  check "reproducible" {
    passthru.bloomery = [
      "NIXLIB-GRAPH-ARCHIVES-004"
      "NIXLIB-GRAPH-ARCHIVES-005"
      "NIXLIB-GRAPH-ARCHIVES-007"
    ];
  } ''
    echo "Archive normal form..."
    for archive in ${a}/meta.tar.zst ${a}/lib.tar.zst ${m}/meta.tar.zst ${d}/meta.tar.zst ${d}/lib.tar.zst; do
      names="$(tar --zstd -tf "$archive")"
      [ "$names" = "$(printf '%s\n' "$names" | LC_ALL=C sort)" ] || fail "$archive members are not sorted"
      TZ=UTC tar --zstd -tvf "$archive" --numeric-owner --full-time | while read -r mode owner size day time name; do
        [ "$owner" = 0/0 ] || fail "$archive member $name has owner $owner"
        [ "$day $time" = "1970-01-01 00:00:01" ] || fail "$archive member $name has mtime $day $time"
      done
    done

    echo "Independent rebuild equality..."
    [ ${d} != ${dRebuilt} ] || fail "rebuild fixture is not an independent derivation"
    for archive in meta.tar.zst lib.tar.zst; do
      cmp ${d}/$archive ${dRebuilt}/$archive || fail "$archive differs between independent builds"
      mkdir -p "first/$archive" "second/$archive"
      tar --zstd -xf ${d}/$archive -C "first/$archive"
      tar --zstd -xf ${dRebuilt}/$archive -C "second/$archive"
    done
    diff -r first second || fail "unpacked archives differ between independent builds"
  ''
