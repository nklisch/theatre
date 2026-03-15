# Design: Auto-Generated Docs — Part 2: VitePress Generation Pipeline

## Overview

This design covers the build-time pipeline that transforms the JSON schema dump
from `theatre-docs-gen` (Part 1) into VitePress markdown content:

1. A **Node.js generation script** that reads `tools.json` and produces markdown fragments
2. **VitePress data loading** via `.data.ts` files that make schema available at build time
3. **Vue components** that render parameter tables and response schemas inline
4. **Build integration** via `npm run docs:generate` pre-build step
5. **CI validation** ensuring docs stay in sync with code

## Implementation Units

### Unit 1: Schema generation script

**File**: `site/scripts/generate-schema.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail
# Generate tool schema JSON from Rust source code.
# Run from repo root: ./site/scripts/generate-schema.sh

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SITE_DIR="$(dirname "$SCRIPT_DIR")"
OUT_DIR="$SITE_DIR/.generated"

mkdir -p "$OUT_DIR"

echo "Building theatre-docs-gen..."
cargo build -p theatre-docs-gen --quiet

echo "Generating tool schemas..."
cargo run -p theatre-docs-gen --quiet > "$OUT_DIR/tools.json"

TOOL_COUNT=$(python3 -c "import json; print(len(json.load(open('$OUT_DIR/tools.json'))))")
echo "Generated schemas for $TOOL_COUNT tools → $OUT_DIR/tools.json"
```

**File**: `site/.generated/.gitignore`

```
# Generated at build time — do not commit
*
!.gitignore
```

**Implementation Notes**:
- The `.generated/` dir is gitignored — schemas are always built fresh from code.
- The script is idempotent and fast (~2s for cargo build if already compiled).
- `python3` is used for the count check only — zero Node.js deps for generation.

**Acceptance Criteria**:
- [ ] `./site/scripts/generate-schema.sh` succeeds from repo root
- [ ] `site/.generated/tools.json` contains valid JSON with all tools
- [ ] Running twice produces identical output (deterministic)

---

### Unit 2: VitePress data loader

VitePress supports [data loading](https://vitepress.dev/guide/data-loading) via `.data.ts` files that export typed data available to any page.

**File**: `site/.vitepress/data/tools.data.ts`

```typescript
import { readFileSync, existsSync } from 'fs'
import { resolve } from 'path'

interface ToolParam {
  name: string
  type: string
  required: boolean
  description: string
  default?: string
  enum?: string[]
}

interface ToolDoc {
  server: 'stage' | 'director'
  name: string
  description: string
  input_schema: {
    type: string
    properties: Record<string, any>
    required?: string[]
    definitions?: Record<string, any>
  }
  output_schema?: {
    type: string
    properties: Record<string, any>
    required?: string[]
  }
}

/**
 * Flatten a JSON Schema property into a ToolParam.
 * Handles $ref resolution, oneOf/anyOf, nested objects.
 */
function schemaToParam(
  name: string,
  prop: any,
  required: string[],
  definitions: Record<string, any>
): ToolParam {
  let resolved = prop
  if (prop.$ref) {
    const refName = prop.$ref.replace('#/definitions/', '')
    resolved = definitions[refName] ?? prop
  }

  let type = resolved.type ?? 'any'
  let enumValues: string[] | undefined

  // Handle nullable (anyOf with null)
  if (resolved.anyOf) {
    const nonNull = resolved.anyOf.filter((s: any) => s.type !== 'null')
    if (nonNull.length === 1) {
      const inner = nonNull[0]
      if (inner.$ref) {
        const refName = inner.$ref.replace('#/definitions/', '')
        const def = definitions[refName]
        if (def?.enum) {
          enumValues = def.enum
          type = 'string'
        } else {
          type = refName
        }
      } else {
        type = inner.type ?? 'any'
        if (inner.enum) enumValues = inner.enum
      }
    }
  }

  if (resolved.enum) enumValues = resolved.enum

  // Array types
  if (type === 'array' && resolved.items) {
    const itemType = resolved.items.type ?? resolved.items.$ref?.replace('#/definitions/', '') ?? 'any'
    type = `${itemType}[]`
  }

  // Integer → number for display
  if (type === 'integer') type = 'number'

  const param: ToolParam = {
    name,
    type,
    required: required.includes(name),
    description: resolved.description ?? prop.description ?? '',
  }

  if (resolved.default !== undefined) {
    param.default = JSON.stringify(resolved.default)
  }

  if (enumValues) {
    param.enum = enumValues
  }

  return param
}

function parseToolParams(tool: ToolDoc): ToolParam[] {
  const schema = tool.input_schema
  if (!schema.properties) return []

  const required = schema.required ?? []
  const definitions = schema.definitions ?? {}

  return Object.entries(schema.properties)
    .map(([name, prop]) => schemaToParam(name, prop, required, definitions))
}

export default {
  watch: ['../../.generated/tools.json'],
  load(): { tools: ToolDoc[]; params: Record<string, ToolParam[]> } {
    const jsonPath = resolve(__dirname, '../../.generated/tools.json')

    if (!existsSync(jsonPath)) {
      console.warn('tools.json not found — run site/scripts/generate-schema.sh first')
      return { tools: [], params: {} }
    }

    const tools: ToolDoc[] = JSON.parse(readFileSync(jsonPath, 'utf-8'))
    const params: Record<string, ToolParam[]> = {}

    for (const tool of tools) {
      params[tool.name] = parseToolParams(tool)
    }

    return { tools, params }
  }
}
```

**Implementation Notes**:
- The `watch` property makes VitePress rebuild when `tools.json` changes during dev.
- `schemaToParam` handles JSON Schema complexities: `$ref` resolution for enum types, `anyOf` for nullable (Option<T>), array items, default values.
- This is the central translation layer between JSON Schema and the display model.
- If `tools.json` doesn't exist, pages degrade gracefully (empty tables, no crash).

**Acceptance Criteria**:
- [ ] Data loader parses all 42 tools without errors
- [ ] Parameter names, types, required status, defaults match actual Rust structs
- [ ] Enum values extracted correctly (e.g., `detail` shows `["summary", "standard", "full"]`)
- [ ] Graceful fallback when `tools.json` is missing

---

### Unit 3: Parameter table Vue component

**File**: `site/.vitepress/theme/components/ParamTable.vue`

```vue
<script setup lang="ts">
defineProps<{
  params: {
    name: string
    type: string
    required: boolean
    description: string
    default?: string
    enum?: string[]
  }[]
}>()
</script>

<template>
  <table class="param-table">
    <thead>
      <tr>
        <th>Parameter</th>
        <th>Type</th>
        <th>Required</th>
        <th>Description</th>
      </tr>
    </thead>
    <tbody>
      <tr v-for="p in params" :key="p.name">
        <td>
          <code>{{ p.name }}</code>
        </td>
        <td>
          <code>{{ p.type }}</code>
          <span v-if="p.enum" class="enum-values">
            <br />
            <small>{{ p.enum.map(v => `"${v}"`).join(' | ') }}</small>
          </span>
        </td>
        <td>
          <span v-if="p.required" class="badge required">required</span>
          <span v-else class="badge optional">
            optional
            <template v-if="p.default">
              <br /><small>default: <code>{{ p.default }}</code></small>
            </template>
          </span>
        </td>
        <td>{{ p.description }}</td>
      </tr>
    </tbody>
  </table>
</template>

<style scoped>
.param-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.9em;
}
.param-table th {
  text-align: left;
  border-bottom: 2px solid var(--vp-c-divider);
  padding: 8px 12px;
}
.param-table td {
  border-bottom: 1px solid var(--vp-c-divider);
  padding: 8px 12px;
  vertical-align: top;
}
.badge {
  display: inline-block;
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 0.8em;
  font-weight: 600;
}
.required {
  background: var(--vp-c-danger-soft);
  color: var(--vp-c-danger-1);
}
.optional {
  background: var(--vp-c-default-soft);
  color: var(--vp-c-text-2);
}
.enum-values {
  color: var(--vp-c-text-3);
}
</style>
```

**File**: `site/.vitepress/theme/components/ResponseSchema.vue`

```vue
<script setup lang="ts">
defineProps<{
  schema: {
    type: string
    properties: Record<string, any>
    required?: string[]
  } | null
}>()

function formatType(prop: any): string {
  if (prop.type === 'array' && prop.items) {
    return `${prop.items.type ?? 'object'}[]`
  }
  return prop.type ?? 'any'
}
</script>

<template>
  <div v-if="schema && schema.properties">
    <table class="param-table">
      <thead>
        <tr>
          <th>Field</th>
          <th>Type</th>
          <th>Description</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="(prop, name) in schema.properties" :key="name">
          <td><code>{{ name }}</code></td>
          <td><code>{{ formatType(prop) }}</code></td>
          <td>{{ prop.description ?? '' }}</td>
        </tr>
      </tbody>
    </table>
  </div>
  <div v-else class="no-schema">
    <em>Response schema not available for this tool.</em>
  </div>
</template>
```

**File**: `site/.vitepress/theme/index.ts` — register components globally

```typescript
import DefaultTheme from 'vitepress/theme'
import ParamTable from './components/ParamTable.vue'
import ResponseSchema from './components/ResponseSchema.vue'
import './custom.css'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component('ParamTable', ParamTable)
    app.component('ResponseSchema', ResponseSchema)
  }
}
```

**Acceptance Criteria**:
- [ ] `<ParamTable>` renders a table with all parameter fields
- [ ] Required/optional badges display correctly
- [ ] Default values shown inline
- [ ] Enum values shown as `"a" | "b" | "c"` below the type
- [ ] `<ResponseSchema>` renders output fields when schema available
- [ ] Graceful empty state when schema missing

---

### Unit 4: Markdown page integration pattern

Each tool page combines human-written prose with generated parameter tables.

**Example**: `site/stage/snapshot.md`

```markdown
---
title: Spatial Snapshot
---

# Spatial Snapshot

<script setup>
import { data } from '../.vitepress/data/tools.data'
const tool = data.tools.find(t => t.name === 'spatial_snapshot')
const params = data.params['spatial_snapshot'] ?? []
</script>

Get a spatial snapshot of the current game scene. Returns entity positions,
states, and spatial relationships relative to a perspective point.

## Parameters

<ParamTable :params="params" />

<!-- Human-written parameter guidance -->
::: tip Choosing a detail level
- `"summary"` (~200 tokens) — clustered overview, best for initial orientation
- `"standard"` (~400-800 tokens) — per-entity data, the default for most queries
- `"full"` (~1000+ tokens) — includes transforms, physics, children, scripts
:::

## Response

<ResponseSchema :schema="tool?.output_schema" />

<!-- Human-written response explanation and examples -->
The response varies by detail tier:

### Summary tier

Returns clustered groups instead of individual entities...

### Standard tier

Returns an `entities` array with per-node data...

## Examples

```json
// Get a standard snapshot from the active camera
{ "detail": "standard" }
```

```json
// Get entities near a specific point
{ "perspective": "point", "focal_point": [10, 0, 5], "radius": 30 }
```
```

**Implementation Notes**:
- The `<script setup>` block imports the data loader and extracts the specific tool.
- `<ParamTable>` and `<ResponseSchema>` are auto-generated from code.
- Everything else (tips, explanations, examples) is human-written.
- This pattern keeps human prose alongside generated tables — best of both worlds.
- If `tools.json` is missing, the page still renders (just with empty tables).

**Acceptance Criteria**:
- [ ] All 13 Stage tool pages use `<ParamTable>` for parameters
- [ ] All 12 Director tool pages use `<ParamTable>` for parameters
- [ ] Parameter tables match actual Rust struct fields exactly
- [ ] Human-written prose preserved around generated tables

---

### Unit 5: Build integration

**File**: `site/package.json` — add generate script

```json
{
  "name": "theatre-docs",
  "private": true,
  "type": "module",
  "scripts": {
    "generate": "cd .. && ./site/scripts/generate-schema.sh",
    "dev": "npm run generate && vitepress dev",
    "build": "npm run generate && vitepress build",
    "preview": "vitepress preview"
  },
  "devDependencies": {
    "vitepress": "^1.6.0",
    "vue": "^3.5.0"
  }
}
```

**Implementation Notes**:
- `npm run generate` runs the Rust binary and dumps `tools.json`.
- `npm run dev` and `npm run build` both generate schemas first, ensuring freshness.
- `npm run preview` skips generation (previews the last build).
- The `cd ..` ensures cargo runs from the workspace root.

**Acceptance Criteria**:
- [ ] `npm run build` (from `site/`) succeeds end-to-end
- [ ] Schema is regenerated on every build
- [ ] `npm run dev` starts with fresh schema and hot-reloads on `.md` changes
- [ ] `site/.generated/tools.json` is never committed to git

---

### Unit 6: CI validation

**File**: `.github/workflows/docs.yml` — add schema validation step

```yaml
# Add after the existing build step:
- name: Validate docs schema
  run: |
    cargo build -p theatre-docs-gen
    cargo run -p theatre-docs-gen > /tmp/tools.json
    python3 -c "
    import json
    tools = json.load(open('/tmp/tools.json'))
    assert len(tools) >= 42, f'Expected 42+ tools, got {len(tools)}'
    for t in tools:
        assert 'name' in t, f'Tool missing name'
        assert 'input_schema' in t, f'{t[\"name\"]} missing input_schema'
        schema = t['input_schema']
        assert 'properties' in schema or schema.get('type') == 'object', \
            f'{t[\"name\"]} schema has no properties'
    print(f'Schema validation passed: {len(tools)} tools')
    "
```

**Acceptance Criteria**:
- [ ] CI fails if `theatre-docs-gen` can't build
- [ ] CI fails if tool count drops (detects accidental tool removal)
- [ ] CI fails if schema structure is malformed

---

### Unit 7: Staleness detection

A script that compares the generated parameter names against what the site docs reference, detecting when a page uses outdated param names.

**File**: `site/scripts/check-staleness.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail
# Check for parameter names in site docs that don't match the generated schema.
# Run after generate-schema.sh.

SITE_DIR="$(cd "$(dirname "$0")/.." && pwd)"
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
```

**Implementation Notes**:
- This is intentionally lightweight. The real staleness protection is that `<ParamTable>` is generated from code — you can't have a wrong param name in the generated table.
- The script validates the schema file exists and is structurally sound.
- Human-written examples in code blocks can still drift — but that's a much smaller surface area than entire parameter tables being fabricated.

**Acceptance Criteria**:
- [ ] Script runs without errors when schema is present
- [ ] Script fails if schema is missing

---

## Implementation Order

1. **Unit 1**: Schema generation script (depends on Part 1 Unit 1: docs-gen binary)
2. **Unit 2**: VitePress data loader
3. **Unit 3**: Vue components (ParamTable, ResponseSchema)
4. **Unit 4**: Update all tool pages to use generated components
5. **Unit 5**: Build integration (package.json)
6. **Unit 6**: CI validation
7. **Unit 7**: Staleness detection

Units 2 and 3 can be done in parallel. Unit 4 is the largest — 25 pages to update.

## Testing

### Dev Server Test
```bash
cd site
npm run dev
# Open http://localhost:5173/theatre/stage/snapshot
# Verify: parameter table shows all 10 params with correct types and defaults
# Verify: enum values visible for detail, perspective
```

### Build Test
```bash
cd site
npm run build
# Should complete without errors
# Output in site/.vitepress/dist/
```

### Visual Regression
- Compare a generated param table against the actual Rust struct definition
- Spot-check 3 tools (one Stage, one Director, one with enums)

## Verification Checklist

```bash
# Full pipeline from code to site
cd /home/nathan/dev/theatre

# 1. Generate schema
./site/scripts/generate-schema.sh

# 2. Verify schema content
python3 -c "
import json
tools = json.load(open('site/.generated/tools.json'))
print(f'{len(tools)} tools')
stage = [t for t in tools if t['server'] == 'stage']
director = [t for t in tools if t['server'] == 'director']
print(f'  Stage: {len(stage)}')
print(f'  Director: {len(director)}')

# Check a specific tool
snap = next(t for t in tools if t['name'] == 'spatial_snapshot')
props = list(snap['input_schema']['properties'].keys())
print(f'  spatial_snapshot params: {props}')
assert 'perspective' in props
assert 'detail' in props
assert 'radius' in props
"

# 3. Build site
cd site && npm run build

# 4. Check built output
ls -la .vitepress/dist/stage/snapshot.html
```

## Migration Path

The page updates (Unit 4) can be done incrementally:

1. **Phase 1**: Add `<ParamTable>` to 5 highest-traffic pages (snapshot, query, inspect, scenes, nodes)
2. **Phase 2**: Add to remaining 20 pages
3. **Phase 3**: Remove the old hand-written parameter tables
4. **Phase 4**: Add `<ResponseSchema>` once output schemas are available (Part 1 Units 4-5)

Each phase is independently deployable — pages with `<ParamTable>` show generated tables, pages without still show the (corrected) hand-written tables.
