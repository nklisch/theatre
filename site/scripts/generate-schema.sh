#!/usr/bin/env bash
set -euo pipefail
# Generate tool schema JSON from Rust source code.
# Run from repo root: ./site/scripts/generate-schema.sh

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
node "$SCRIPT_DIR/generate-schema.mjs"
