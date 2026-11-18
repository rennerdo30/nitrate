#!/usr/bin/env bash
# Checks that the workspace crates are ready for crates.io, without uploading anything:
# - the exact versions of the workspace crates in [workspace.dependencies] match the
#   workspace version;
# - each crate's LICENSE copy matches the root LICENSE (crates can only ship their own files);
# - `cargo package` builds every crate from its packaged sources alone;
# - each .crate stays under crates.io's 10 MB limit.
# Run by CI (job "crates") and by the publish workflow before any upload.
set -euo pipefail
cd "$(dirname "$0")/.."

version=$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' Cargo.toml)
[ -n "$version" ] || { echo "no workspace version" >&2; exit 1; }
for crate in nitrate-mpv nitrate-ui nitrate-update; do
  pinned=$(sed -n "s/^$crate = { .*version = \"=\([^\"]*\)\".*/\1/p" Cargo.toml)
  if [ "$pinned" != "$version" ]; then
    echo "$crate is pinned to '$pinned' in [workspace.dependencies], workspace version is $version" >&2
    exit 1
  fi
done
for dir in crates/*/; do
  cmp -s LICENSE "${dir}LICENSE" || { echo "${dir}LICENSE differs from LICENSE" >&2; exit 1; }
done

# NITRATE_ALLOW_DIRTY=1 lets a local run include uncommitted changes; CI never sets it.
cargo package --workspace --locked ${NITRATE_ALLOW_DIRTY:+--allow-dirty}

target=$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p' | sed 's/\\/\//g')
for f in "$target"/package/*-"$version".crate; do
  size=$(wc -c < "$f")
  echo "$(basename "$f"): $size bytes"
  [ "$size" -lt 10000000 ] || { echo "$f is over 10 MB" >&2; exit 1; }
done
