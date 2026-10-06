#!/usr/bin/env bash
# Apply one scenario mutation and time a builder command.
set -euo pipefail

if [[ ! -d "$PWD/benchmarks" ]]; then
  echo "run the benchmark from the repository root" >&2
  exit 2
fi

exec python3 "$PWD/benchmarks/harness/harness.py" measure "$@"
