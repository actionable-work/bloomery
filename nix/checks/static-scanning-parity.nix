{
  pkgs,
  workspace,
}: let
  dependencies = import ../../lib/builders/dependencies.nix {lib = pkgs.lib;} {};
in
  pkgs.runCommand "bloomery-static-scanning-parity" {
    nativeBuildInputs = [pkgs.rustc pkgs.stdenv.cc pkgs.zstd pkgs.python3];
    passthru.bloomery = ["PARSER-SCANNING-TREES-001" "PARSER-WORKSPACE-TREES-008" "PARSER-WORKSPACE-REPORT-001" "PARSER-WORKSPACE-REPORT-008" "PARSER-WORKSPACE-REPORT-010" "PARSER-WORKSPACE-REPORT-011" "PARSER-WORKSPACE-REPORT-012" "PARSER-WORKSPACE-REPORT-013" "PARSER-WORKSPACE-REPORT-015" "PARSER-WORKSPACE-REPORT-016" "CLI-SPEC-TRACE-017" "CLI-SPEC-TRACE-018" "CLI-SPEC-TRACE-019"];
  } ''
    ${dependencies}
    declare -a EXTERN_FLAGS=() LINK_FLAGS=()
    resolve_deps EXTERN_FLAGS LINK_FLAGS "$PWD/deps" \
      ${workspace.crates."bloomery-scanning-0.1.0"} \
      ${workspace.crates."bloomery-model-0.1.0"} \
      ${workspace.crates."bloomery-workspace-0.1.0"} \
      ${workspace.crates."bloomery-check-0.1.0"} \
      ${workspace.crates."serde_json-1.0.151"}
    rustc --edition=2024 ${../../tests/static-scanning-parity/consumer.rs} \
      -L dependency="$PWD/deps" "''${EXTERN_FLAGS[@]}" "''${LINK_FLAGS[@]}" -o consumer
    mkdir "$out"
    python ${../../tests/static-scanning-parity}/check.py \
      ${workspace.packages.bloomery}/bin/bloomery "$PWD/consumer" > "$out/report.txt"
  ''
