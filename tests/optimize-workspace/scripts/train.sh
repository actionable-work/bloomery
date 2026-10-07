#!/usr/bin/env bash
# Exercises the instrumented binary so it writes profile data to
# BLOOMERY_PROFILE_DIR, then exits so the runtime flushes its profiles.
set -euo pipefail

"$BLOOMERY_TRAIN_BINARY"
