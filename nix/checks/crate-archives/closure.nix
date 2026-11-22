{
  pkgs,
  fixture,
}: let
  inherit (fixture) a m b c d check;
in
  check "closure" {
    passthru.bloomery = [
      "NIXLIB-GRAPH-DERIVATIONS-030"
      "NIXLIB-GRAPH-ARCHIVES-008"
      "NIXLIB-GRAPH-ARCHIVES-009"
    ];
    closure = pkgs.writeClosure [d];
  } ''
    echo "Closures and references..."
    [ "$(cat ${m}/nix-support/deps-closure)" = ${m} ] || fail "proc-macro closure lists more than itself"
    expected="$(printf '%s\n' ${a} ${b} ${c} ${d} ${m} | LC_ALL=C sort)"
    [ "$(cat ${d}/nix-support/deps-closure)" = "$expected" ] || fail "d closure is not {a, b, c, d, m}"
    unexpected="$(LC_ALL=C comm -23 <(LC_ALL=C sort "$closure") ${d}/nix-support/deps-closure)"
    [ -z "$unexpected" ] || fail "d references paths outside its closure: $unexpected"
    if grep -q '/build' ${d}/nix-support/meta.sh ${d}/nix-support/deps-closure; then
      fail "d metadata names a build-local path"
    fi
  ''
