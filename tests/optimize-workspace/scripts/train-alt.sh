#!/usr/bin/env bash
# Alternate training script used to prove training-only invalidation.
set -euo pipefail

"$BLOOMERY_TRAIN_BINARY"
"$BLOOMERY_TRAIN_BINARY"
