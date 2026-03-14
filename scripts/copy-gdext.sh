#!/usr/bin/env bash
# Copy the built GDExtension library to the addon directory.
# Usage: ./scripts/copy-gdext.sh [debug|release]

set -euo pipefail

MODE="${1:-debug}"
SRC="target/${MODE}/libstage_godot.so"
DST="addons/stage/bin/linux/"

if [ ! -f "$SRC" ]; then
    echo "Error: $SRC not found. Run 'cargo build -p stage-godot' first."
    exit 1
fi

mkdir -p "$DST"
cp "$SRC" "$DST"
echo "Copied $SRC → $DST"
