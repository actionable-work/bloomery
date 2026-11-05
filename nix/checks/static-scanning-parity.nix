{
  pkgs,
  workspace,
}: let
  dependencies = import ../../lib/builders/dependencies.nix {lib = pkgs.lib;} {};
in
  pkgs.runCommand "bloomery-static-scanning-parity" {
    nativeBuildInputs = [pkgs.rustc pkgs.stdenv.cc pkgs.zstd pkgs.python3];
    passthru.bloomery = ["PARSER-SCANNING-TREES-001" "PARSER-WORKSPACE-TREES-008"];
  } ''
    ${dependencies}
    declare -a EXTERN_FLAGS=() LINK_FLAGS=()
    resolve_deps EXTERN_FLAGS LINK_FLAGS "$PWD/deps" \
      ${workspace.crates."bloomery-scanning-0.1.0"} \
      ${workspace.crates."bloomery-model-0.1.0"} \
      ${workspace.crates."bloomery-workspace-0.1.0"} \
      ${workspace.crates."bloomery-check-0.1.0"}
    rustc --edition=2024 ${../../tests/static-scanning-parity/consumer.rs} \
      -L dependency="$PWD/deps" "''${EXTERN_FLAGS[@]}" "''${LINK_FLAGS[@]}" -o consumer
    mkdir "$out"
    python ${../../tests/static-scanning-parity}/check.py \
      ${workspace.packages.bloomery}/bin/bloomery "$PWD/consumer" > "$out/report.txt"
  ''
