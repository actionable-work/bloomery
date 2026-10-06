#!/usr/bin/env bash
# Run the Bloomery benchmark suite from the repository root.
set -euo pipefail

if [[ ! -f "$PWD/flake.nix" || ! -d "$PWD/benchmarks" ]]; then
  echo "run the benchmark from the repository root" >&2
  exit 2
fi

exec python3 "$PWD/benchmarks/harness/harness.py" run "$@"
