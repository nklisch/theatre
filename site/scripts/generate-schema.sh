#!/usr/bin/env bash
set -euo pipefail
# Generate tool schema JSON from Rust source code.
# Run from repo root: ./site/scripts/generate-schema.sh

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SITE_DIR="$(dirname "$SCRIPT_DIR")"
REPO_DIR="$(dirname "$SITE_DIR")"
OUT_DIR="$SITE_DIR/.generated"

mkdir -p "$OUT_DIR"

cd "$REPO_DIR"

echo "Building theatre-docs-gen..."
cargo build -p theatre-docs-gen --quiet

echo "Generating tool schemas..."
cargo run -p theatre-docs-gen --quiet > "$OUT_DIR/tools.json"

TOOL_COUNT=$(python3 -c "import json; print(len(json.load(open('$OUT_DIR/tools.json'))))")
echo "Generated schemas for $TOOL_COUNT tools → $OUT_DIR/tools.json"
