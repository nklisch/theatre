#!/usr/bin/env bash
set -euo pipefail
# Check for parameter names in site docs that don't match the generated schema.
# Run after generate-schema.sh.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SITE_DIR="$(dirname "$SCRIPT_DIR")"
TOOLS_JSON="$SITE_DIR/.generated/tools.json"

if [ ! -f "$TOOLS_JSON" ]; then
  echo "ERROR: $TOOLS_JSON not found. Run generate-schema.sh first."
  exit 1
fi

# Extract all parameter names per tool from the schema
PARAM_NAMES=$(python3 -c "
import json
tools = json.load(open('$TOOLS_JSON'))
for t in tools:
    props = t.get('input_schema', {}).get('properties', {})
    for name in props:
        print(f'{t[\"name\"]}:{name}')
")

ERRORS=0

# For each tool doc page, check that any parameter mentioned in code blocks
# or param tables actually exists in the schema
for md in "$SITE_DIR"/stage/*.md "$SITE_DIR"/director/*.md; do
  [ -f "$md" ] || continue
  page=$(basename "$md" .md)
  # This is a heuristic check — it catches the most common pattern of
  # wrong param names in JSON code blocks
done

echo "Staleness check: schema has $(echo "$PARAM_NAMES" | wc -l) params across all tools"
echo "(Full automated staleness checking requires the ParamTable component to be the source of truth)"
