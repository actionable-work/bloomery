{
  pkgs,
  root ? ../..,
}: let
  harness = root + "/benchmarks/harness";
  testdata = harness + "/testdata";
in
  pkgs.runCommand "bloomery-benchmarks-report" {
    nativeBuildInputs = [pkgs.python3];
    passthru.bloomery = [
      "REPOSITORY-BENCHMARKS-RESULTS-001"
      "REPOSITORY-BENCHMARKS-RESULTS-002"
      "REPOSITORY-BENCHMARKS-RESULTS-003"
      "REPOSITORY-BENCHMARKS-RESULTS-004"
      "REPOSITORY-BENCHMARKS-RESULTS-005"
      "REPOSITORY-BENCHMARKS-RESULTS-006"
      "REPOSITORY-BENCHMARKS-HARNESS-002"
      "REPOSITORY-BENCHMARKS-HARNESS-003"
      "REPOSITORY-BENCHMARKS-HARNESS-008"
      "REPOSITORY-BENCHMARKS-HARNESS-009"
      "REPOSITORY-BENCHMARKS-RESULTS-007"
      "REPOSITORY-BENCHMARKS-RESULTS-008"
      "REPOSITORY-BENCHMARKS-RESULTS-009"
      "REPOSITORY-BENCHMARKS-RESULTS-010"
      "REPOSITORY-BENCHMARKS-RESULTS-011"
    ];
  } ''
    export PYTHONDONTWRITEBYTECODE=1
    mkdir -p work/raw
    cp ${testdata}/meta.json work/meta.json
    cp ${testdata}/simple.bloomery.no-change.build.json work/raw/
    cp ${testdata}/simple.bloomery.no-change.check.json work/raw/
    cp ${testdata}/simple.crane.no-change.build.json work/raw/
    cp ${testdata}/simple.naersk.no-change.check.json work/raw/

    python3 ${harness}/report.py --results-dir work --meta work/meta.json

    test -f work/current.md
    test -f work/history.json
    grep -q "CPU" work/current.md
    grep -q "Memory" work/current.md
    grep -q "Ratio to Bloomery" work/current.md
    grep -q "crane" work/current.md
    grep -q "### simple" work/current.md
    grep -q "## Feature matrix" work/current.md
    grep -q "per-crate" work/current.md
    grep -q "workspace" work/current.md
    grep -qE "x\\*" work/current.md
    grep -q "Test cases" work/current.md
    grep -q "Unmodified workspace build" work/current.md
    grep -q "Check coverage" work/current.md
    grep -q "package build only" work/current.md
    grep -q "Single binary and one library" work/current.md

    python3 - <<'PY'
    import json

    data = json.load(open("work/history.json"))
    assert len(data["runs"]) == 1, data
    entry = data["runs"][0]
    for key in ("timestamp", "machine", "environment", "results"):
        assert key in entry, key
    assert entry["machine"]["cpu_model"] == "Benchmark CPU Model", entry
    assert entry["machine"]["memory_bytes"] > 0, entry
    assert entry["environment"]["system"] == "x86_64-linux", entry
    assert any(
        item["builder"] == "bloomery" and item["scenario"] == "no-change"
        for item in entry["results"]
    )
    PY

    python3 ${harness}/report.py --results-dir work --meta work/meta.json

    python3 - <<'PY'
    import json

    data = json.load(open("work/history.json"))
    assert len(data["runs"]) == 2, data
    PY

    mkdir "$out"
    echo "passed" > "$out/success"
  ''
