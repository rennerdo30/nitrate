#!/usr/bin/env bash
# Build and start Nitrate (macOS, Linux). Playback is Windows-only so far;
# this builds and runs the same binary, which says so.
#   scripts/run.sh [--release] [FILE]
set -euo pipefail
cd "$(dirname "$0")/.."
release=() args=()
for arg in "$@"; do
  case "$arg" in
    --release) release=(--release) ;;
    *) args+=("$arg") ;;
  esac
done
cpus=$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)
jobs=$(( cpus / 2 > 0 ? cpus / 2 : 1 )); [ "$jobs" -gt 8 ] && jobs=8
exec cargo run "${release[@]}" -j "$jobs" --quiet -p nitrate -- "${args[@]}"
