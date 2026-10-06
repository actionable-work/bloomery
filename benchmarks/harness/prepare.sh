#!/usr/bin/env bash
# Reset the shared workspace to its committed baseline before a timed run.
set -euo pipefail

if [[ ! -d "$PWD/benchmarks" ]]; then
  echo "run the benchmark from the repository root" >&2
  exit 2
fi

exec python3 "$PWD/benchmarks/harness/harness.py" prepare "$@"
