#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "==> Running checks for all test workspace flakes..."
found=0
for flake_dir in "$SCRIPT_DIR"/*/; do
  if [ -f "$flake_dir/flake.nix" ]; then
    found=$((found + 1))
    name="$(basename "$flake_dir")"
    echo "--> Checking test flake: $name..."
    nix flake check --print-build-logs "$flake_dir"
  fi
done

if [ "$found" -eq 0 ]; then
  echo "No test workspace flakes found."
  exit 1
fi

echo "==> All $found test workspace flakes passed checks!"
