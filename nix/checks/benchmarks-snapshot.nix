{
  pkgs,
  root ? ../..,
}: let
  harness = root + "/benchmarks/harness";
in
  pkgs.runCommand "bloomery-benchmarks-snapshot" {
    nativeBuildInputs = [pkgs.python3 pkgs.git];
    passthru.bloomery = [
      "REPOSITORY-BENCHMARKS-HARNESS-017"
      "REPOSITORY-BENCHMARKS-HARNESS-018"
      "REPOSITORY-BENCHMARKS-HARNESS-019"
      "REPOSITORY-BENCHMARKS-HARNESS-027"
      "REPOSITORY-BENCHMARKS-HARNESS-029"
    ];
  } ''
    export PYTHONDONTWRITEBYTECODE=1
    python3 ${harness}/test_harness.py

    mkdir "$out"
    echo "passed" > "$out/success"
  ''
