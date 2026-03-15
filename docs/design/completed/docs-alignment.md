# Design: Documentation Alignment — Zero Stage, Accurate Schemas

## Overview

Comprehensive fix for all documentation discrepancies found during the audit.
Two categories of work:

1. **Eliminate every "stage" reference** — site, CLAUDE.md, docs/, .claude/.
   The codebase was renamed from `stage-*` to `stage-*`. The binary is
   `stage`, the addon is `addons/stage/`, the crates are `stage-server`,
   `stage-godot`, `stage-protocol`, `stage-core`. Zero legacy references remain.

2. **Align API docs to actual implementation** — parameter names, types,
   defaults, response shapes, action types, port numbers.

---

## Unit 1: Purge "stage" from CLAUDE.md

**File**: `CLAUDE.md`

Every occurrence of `stage` (case-insensitive) must be replaced with the
correct `stage` equivalent. This includes crate names, binary names, addon
paths, class names, and prose references.

**Replacements** (line → old → new):

| Line(s) | Old | New |
|---------|-----|-----|
| 6 | `**Stage**: Rust MCP server + Rust GDExtension addon` | `**Stage**: Rust MCP server + Rust GDExtension addon` |
| 15 | `stage-server/     — Stage MCP binary` | `stage-server/        — Stage MCP binary` |
| 16 | `stage-godot/      — Stage GDExtension cdylib` | `stage-godot/         — Stage GDExtension cdylib` |
| 17 | `stage-protocol/   — Shared TCP wire format types` | `stage-protocol/      — Shared TCP wire format types` |
| 18 | `stage-core/       — Shared spatial logic` | `stage-core/          — Shared spatial logic` |
| 21 | `addons/stage/       — Stage Godot addon` | `addons/stage/          — Stage Godot addon` |
| 28 | `wire-tests/           — Stage E2E tests` | `wire-tests/           — Stage E2E tests` |
| 39 | `cargo build -p stage-server` | `cargo build -p stage-server` |
| 40 | `cargo build -p stage-godot` | `cargo build -p stage-godot` |
| 47 | `theatre deploy ~/dev/stage/tests/godot-project` | `theatre deploy ~/dev/theatre/tests/godot-project` |
| 97 | `Stage TCP server starts` | `Stage TCP server starts` |
| 102 | `` `stage-godot` targets `api-4-5` `` | `` `stage-godot` targets `api-4-5` `` |
| 105 | `classes stage never uses` | `classes Stage never uses` |
| 106 | `` `stage.gdextension` `` | `` `stage.gdextension` `` |
| 108-109 | `crates/stage-godot/Cargo.toml` | `crates/stage-godot/Cargo.toml` |
| 121-123 | `stage serve`, `stage <tool>` | `stage serve`, `stage <tool>` |
| 124 | `stage-godot runs on Godot's main thread` | `stage-godot runs on Godot's main thread` |
| 137 | `stage-server` | `stage-server` |
| 145 | `### Stage` | `### Stage` |
| 146-148 | `stage-godot depends on stage-protocol` etc. | `stage-godot depends on stage-protocol` etc. |
| 175 | `add StageTCPServer handshake` | `add StageTCPServer handshake` |

**Acceptance Criteria**:
- [ ] `grep -ci stage CLAUDE.md` returns 0
- [ ] All crate names reference `stage-*`
- [ ] Binary name is `stage` not `stage`
- [ ] Addon path is `addons/stage/`
- [ ] Build commands use `-p stage-server`, `-p stage-godot`

---

## Unit 2: Purge "stage" from site source files

**Files** (8 remaining occurrences across 4 files):

### `site/index.md` (line 128)
```
Old: via MCP tools or CLI (`stage spatial_snapshot '{"detail":"summary"}'`)
New: via MCP tools or CLI (`stage spatial_snapshot '{"detail":"summary"}'`)
```

### `site/guide/what-is-theatre.md` (lines 38-39)
```
Old: (`addons/stage/`) ... (`stage`) ... (`stage serve`) ... (`stage <tool> '<json>'`)
New: (`addons/stage/`) ... (`stage`) ... (`stage serve`) ... (`stage <tool> '<json>'`)
```

### `site/guide/how-it-works.md` (lines 46, 61-63, 124)
```
Old: `addons/stage/plugin.gd`
New: `addons/stage/plugin.gd`

Old: The Stage server (`stage` binary, crate: `stage-server`)
New: The Stage server (`stage` binary, crate: `stage-server`)

Old: `stage serve` / `stage <tool>`
New: `stage serve` / `stage <tool>`

Old: Both Stage (`stage serve`) and Director (`director serve`)
New: Both Stage (`stage serve`) and Director (`director serve`)
```

### `site/guide/mcp-basics.md` (line 61)
```
Old: ./target/release/stage serve
New: ./target/release/stage serve
```

### `site/api/wire-format.md` (line 178)
```
Old: `crates/stage-protocol/src/codec.rs`
New: `crates/stage-protocol/src/codec.rs`
```

**Acceptance Criteria**:
- [ ] `grep -rci stage site/ --include='*.md' --include='*.vue' --include='*.mts' --include='*.ts' --include='*.css'` returns 0
- [ ] All CLI examples use `stage` binary name
- [ ] All crate/path references use `stage-*`

---

## Unit 3: Purge "stage" from docs/ directory

**Files**: All markdown files in `docs/` and `docs/design/` (both active and
completed). The user explicitly requested zero "stage" references even in
historical/archived docs.

**Approach**: For each file in `docs/`, replace:
- `stage-server` → `stage-server`
- `stage-godot` → `stage-godot`
- `stage-protocol` → `stage-protocol`
- `stage-core` → `stage-core`
- `Stage` (capitalized, product name) → `Stage`
- `stage` (lowercase standalone, not part of compound) → `stage`
- `StageTCPServer` → `StageTCPServer`
- `StageCollector` → `StageCollector`
- `StageRecorder` → `StageRecorder`
- `StageExtension` → `StageExtension`
- `StageServer` → `StageServer`
- `StageRuntime` → `StageRuntime`
- `StageContributors` → `StageContributors`
- `addons/stage/` → `addons/stage/`
- `stage.gdextension` → `stage.gdextension`
- `libstage_godot` → `libstage_godot`
- `/stage/` (URL paths) → `/stage/`
- `STAGE_PORT` → `STAGE_PORT`
- `is_stage_node` → `is_stage_node`
- `theatre/stage/` (settings prefix) → `theatre/stage/`

**Files with known matches** (from audit):
- `docs/THEATRE-MIGRATION.md`
- `docs/DIRECTOR-ROADMAP.md`
- `docs/design/stage-rename-and-input.md`
- `docs/design/completed/M0-SKELETON.md`
- `docs/design/completed/M8-RECORDING-ANALYSIS.md`
- `docs/design/completed/M9-2D-SUPPORT.md`
- `docs/design/completed/github-pages.md`
- `docs/design/completed/theatre-rename.md`
- Any others found by grep

**Implementation Note**: Run `grep -rli stage docs/` to find all files,
then apply replacements file by file. Some design docs may have tables mapping
old→new names (like `stage-rename-and-input.md`); update both columns so the
table reads "was X (renamed to Y)" or simply uses the new names throughout.

**Acceptance Criteria**:
- [ ] `grep -rci stage docs/` returns 0
- [ ] Completed design docs still make sense (context preserved, just names updated)

---

## Unit 4: Fix Director port numbers

The docs have editor and daemon ports **reversed**.

**Actual code** (verified):
- Editor plugin: port **6551** (`crates/director/src/editor.rs:9`)
- Daemon: port **6550** (`crates/director/src/daemon.rs:11`)

**Files to fix**:

### `site/guide/how-it-works.md`
```
Old: **Editor plugin backend** (port 6550)
New: **Editor plugin backend** (port 6551)

Old: **Headless daemon backend** (port 6551)
New: **Headless daemon backend** (port 6550)

Old: it tries port 6550, then port 6551
New: it tries port 6551, then port 6550

Old: TCP :6550/:6551
New: TCP :6551/:6550
```

### `site/api/wire-format.md`
```
Old: Director (editor plugin) | `6550`
New: Director (editor plugin) | `6551`

Old: Director (headless daemon) | `6551`
New: Director (headless daemon) | `6550`
```

### `site/director/index.md` (if it mentions ports)
Check and fix any port references.

### `site/director/editor-backend.md` and `site/director/daemon.md`
Check and fix any port references.

### `site/api/errors.md`
```
Old: Port 6550/6551
New: Port 6551/6550 (with correct assignment)
```

### `CLAUDE.md` (Architecture Rules → Director section, if it mentions ports)
Verify port references.

### `site/.vitepress/theme/components/ArchDiagram.vue`
```
Old: TCP :6550/:6551
New: TCP :6551/:6550
```

**Acceptance Criteria**:
- [ ] Every doc that mentions Director ports shows editor=6551, daemon=6550
- [ ] ArchDiagram.vue shows correct ports

---

## Unit 5: Rewrite Stage API reference (`site/api/index.md`)

Complete rewrite of the Stage API reference to match actual parameter structs.

### `spatial_snapshot`

```typescript
{
  perspective?: "camera" | "node" | "point"  // default: "camera"
  focal_node?: string           // required when perspective="node"
  focal_point?: number[]        // required when perspective="point"
  radius?: number               // default: 50.0
  detail?: "summary" | "standard" | "full"  // default: "standard"
  groups?: string[]             // filter by group membership
  class_filter?: string[]       // filter by Godot class
  include_offscreen?: boolean   // default: false
  token_budget?: number         // default: per-detail (summary=500, standard=1500, full=3000)
  expand?: string               // drill into a cluster
}
```

### `spatial_delta`

```typescript
{
  perspective?: "camera" | "node" | "point"  // default: "camera"
  radius?: number               // default: 50.0
  groups?: string[]
  class_filter?: string[]
  token_budget?: number
}
```

**Implementation Note**: Delta compares against a stored baseline from the
previous snapshot — there is NO `since_frame` parameter. Document that the
agent should call `spatial_snapshot` first to establish a baseline, then
`spatial_delta` returns what changed since that baseline.

### `spatial_query`

```typescript
{
  query_type: "nearest" | "radius" | "area" | "raycast" | "path_distance" | "relationship"
  from: string | number[]       // node path or position
  to?: string | number[]        // for path_distance, relationship
  k?: number                    // default: 5 (not 10)
  radius?: number               // default: 20.0
  groups?: string[]
  class_filter?: string[]
  token_budget?: number
}
```

**Response for `relationship`** (fix field names):
```typescript
{
  distance: number
  bearing_from_a: number
  bearing_from_b: number
  line_of_sight: boolean
  elevation_diff?: number
  occluder?: string
  nav_distance?: number
}
```

**Response for `path_distance`** (fix field names):
```typescript
{
  nav_distance: number
  straight_distance: number
  path_ratio: number
  path_points: number[][]
  traversable: boolean
}
```

### `spatial_inspect`

```typescript
{
  node: string                  // required
  include?: Array<"transform" | "physics" | "state" | "children" | "signals" | "script" | "spatial_context" | "resources">
  // default: ["transform", "physics", "state", "children", "signals", "script", "spatial_context"]
}
```

### `spatial_watch`

```typescript
{
  action: "add" | "remove" | "list" | "clear"  // not "create"/"delete"
  watch?: {
    node: string
    conditions?: Array<{
      property: string
      operator: "lt" | "gt" | "eq" | "changed"
      value?: any
    }>
    track?: Array<"position" | "state" | "signals" | "physics" | "all">  // default: ["all"]
  }
  watch_id?: string             // for "remove"
}
```

### `spatial_config`

```typescript
{
  static_patterns?: string[]
  state_properties?: { [class_name: string]: string[] }
  cluster_by?: "group" | "class" | "proximity" | "none"
  bearing_format?: "cardinal" | "degrees" | "both"
  expose_internals?: boolean
  poll_interval?: number
  token_hard_cap?: number
}
```

**Implementation Note**: The old docs described a completely different config
system (tick_rate, capture_radius, etc.). The actual config controls MCP
behavior — clustering, bearing format, token limits. The addon-side collection
config (tick rate, capture radius, buffer depth) is not exposed via this tool.

### `spatial_action`

```typescript
{
  action: "pause" | "advance_frames" | "advance_time" | "teleport"
        | "set_property" | "call_method" | "emit_signal"
        | "spawn_node" | "remove_node"
        | "action_press" | "action_release"
        | "inject_key" | "inject_mouse_button"
  node?: string
  // pause
  paused?: boolean
  // advance_frames
  frames?: number
  // advance_time
  seconds?: number
  // teleport
  position?: number[]
  rotation_deg?: number
  // set_property
  property?: string
  value?: any
  // call_method
  method?: string
  args?: any[]
  // emit_signal
  signal?: string
  // spawn_node
  scene_path?: string
  parent?: string
  name?: string
  // action_press / action_release
  input_action?: string
  strength?: number
  // inject_key
  keycode?: string
  pressed?: boolean
  echo?: boolean                // default: false
  // inject_mouse_button
  button?: string
  // return delta after action
  return_delta?: boolean        // default: false
}
```

### `scene_tree`

```typescript
{
  action: "roots" | "children" | "subtree" | "ancestors" | "find"
  node?: string
  depth?: number                // default: 3
  find_by?: "name" | "class" | "group" | "script"
  find_value?: string
  include?: Array<"class" | "groups" | "script" | "visible" | "process_mode">
  // default: ["class", "groups"]
  token_budget?: number
}
```

### `clips`

Keep current structure but verify action names match the `ClipAction` enum:
`add_marker`, `save`, `status`, `list`, `delete`, `markers`, `snapshot_at`,
`trajectory`, `query_range`, `diff_frames`, `find_event`, `screenshot_at`,
`screenshots`. These match.

**Acceptance Criteria**:
- [ ] Every parameter struct matches the actual `#[derive(Deserialize, JsonSchema)]` struct
- [ ] Default values match `#[serde(default = "...")]` functions
- [ ] Response field names match actual serialization
- [ ] No phantom parameters (documented but not in code)
- [ ] No missing parameters (in code but not documented)

---

## Unit 6: Rewrite Director API reference (`site/api/director.md`)

Complete rewrite using actual parameter field names from the code.

**Key naming pattern**: All operations use full names — `scene_path` (not
`scene`), `node_path` (not `node`), `node_type` (not `class`), `resource_path`
(not `path` or `save_path`).

### Scene operations — field renames

| Documented | Actual |
|-----------|--------|
| `path` | `scene_path` |
| `root_class` | `root_type` |
| `root_name` | *(remove — doesn't exist)* |
| `max_depth` | `depth` |
| `scene` (in scene_add_instance) | `scene_path` |
| `parent` | `parent_path` |
| `source_scene` | `instance_scene` |
| `name` | `node_name` |
| `position` (in scene_add_instance) | *(remove — doesn't exist)* |

### Node operations — field renames

| Documented | Actual |
|-----------|--------|
| `scene` | `scene_path` |
| `node` | `node_path` |
| `parent` | `parent_path` |
| `name` | `node_name` |
| `class` | `node_type` |
| `position` (in node_add) | *(remove — doesn't exist)* |
| `new_parent` | `new_parent_path` |
| `class` (in node_find) | `class_name` |
| `script` (in node_set_script) | `script_path` |
| *(missing)* | `limit: 100` (add to node_find) |

### Resource operations — field renames

| Documented | Actual |
|-----------|--------|
| `path` (resource_read) | `resource_path` |
| `properties?: string[]` (resource_read) | *(remove — doesn't exist)* |
| *(missing)* | `depth: 1` (add to resource_read) |
| `save_path` (material_create) | `resource_path` |
| *(missing)* | `shader_path` (add to material_create) |
| `properties` (shape_create) | `shape_params` |
| *(missing)* | `scene_path`, `node_path` (add to shape_create) |
| `save_path` (style_box_create) | `resource_path` |
| *(missing)* | `property_overrides`, `deep_copy` (add to resource_duplicate) |

### TileMap operations — field renames

| Documented | Actual |
|-----------|--------|
| `scene` | `scene_path` |
| `node` | `node_path` |
| `layer` | *(remove — doesn't exist)* |
| `cells[].position` | `cells[].coords` |
| *(missing)* | `cells[].alternative_tile` |
| `region: { min, max }` | `region: { position, size }` |

### GridMap operations — field renames

| Documented | Actual |
|-----------|--------|
| `scene` | `scene_path` |
| `node` | `node_path` |

### Animation operations — complete rewrite

The animation API operates on standalone `.tres` resource files, NOT on
AnimationPlayer nodes in scenes. Major structural change:

| Documented | Actual |
|-----------|--------|
| `scene` + `node` + `animation_name` | `resource_path` (path to .tres file) |
| `track_path` | `node_path` |
| `track_type: "property"` etc. | `track_type: "value"`, `"position_3d"`, `"rotation_3d"`, etc. |
| `keyframes[].easing` | `keyframes[].transition` |
| *(missing)* | `interpolation`, `update_mode` |
| *(missing)* | `step` (in animation_create) |
| *(missing ops)* | `animation_read`, `animation_remove_track` |

### Physics operations — restructure

Replace 3 documented operations with the actual 2:

```
Old: physics_layer_names, physics_layer_set, physics_mask_set
New: physics_set_layers, physics_set_layer_names
```

`physics_set_layers` takes `collision_layer` and `collision_mask` as optional
u32 bitmasks — no separate layer-set/mask-set operations.

`physics_set_layer_names` requires `layer_type` (e.g., `"3d_physics"`).

### Signal operations — field renames

| Documented | Actual |
|-----------|--------|
| `scene` | `scene_path` |
| `from_node` | `source_path` |
| `signal` | `signal_name` |
| `to_node` | `target_path` |
| `method` | `method_name` |
| `node` (signal_list) | `node_path` (optional) |

### Shader operations — expand

`visual_shader_create` needs full documentation of `nodes` and `connections`
arrays with their struct shapes.

### Export/UID operations — field renames

| Documented | Actual |
|-----------|--------|
| `path` (uid_get) | `file_path` |
| `scene` (export_mesh_library) | `scene_path` |
| `save_path` (export_mesh_library) | `output_path` |
| *(missing)* | `items` (optional filter in export_mesh_library) |
| *(missing)* | `directory` (optional in uid_update_project) |

**Acceptance Criteria**:
- [ ] Every parameter name matches actual struct field name
- [ ] No phantom parameters
- [ ] All 38 operations documented (add `animation_read`, `animation_remove_track`)
- [ ] Physics section shows 2 operations, not 3
- [ ] Animation section uses `resource_path` model, not scene+node model

---

## Unit 7: Fix Stage tool subpages (`site/stage/*.md`)

These pages have auto-generated ParamTable components that read from
`tools.data`, so they may already be correct IF the data source is generated
from code. But the **prose sections** contain wrong information.

### `site/stage/delta.md`
- Remove `since_frame` prose and examples (parameter doesn't exist)
- Remove `min_distance_change` and `min_velocity_change` prose
- Add explanation of baseline-based delta model
- Fix response format (remove `from_frame`/`to_frame`/`elapsed_ms`/
  `changed_node_count`/`unchanged_node_count` keyed-object structure if it
  doesn't match actual response)

### `site/stage/config.md`
- Remove `tick_rate`, `capture_radius`, `capture_center`, `tracked_types`,
  `extra_tracked_types`, `buffer_depth_frames` prose
- Replace with `static_patterns`, `state_properties`, `cluster_by`,
  `bearing_format`, `expose_internals`, `poll_interval`, `token_hard_cap`
- Fix response format example

### `site/stage/action.md`
- Add documentation for all 13 action types (currently only shows 3)
- Add `pause`, `advance_frames`, `advance_time`, `teleport`, `spawn_node`,
  `remove_node`, `action_press`, `action_release`, `inject_key`,
  `inject_mouse_button`
- Document `return_delta` parameter
- Update "When to use it" section to cover input injection and game control

### Other stage pages
- Verify `site/stage/snapshot.md`, `site/stage/query.md`, `site/stage/inspect.md`,
  `site/stage/watch.md`, `site/stage/scene-tree.md`, `site/stage/recording.md`
  against actual params. The ParamTable may be auto-correct, but verify prose.

**Acceptance Criteria**:
- [ ] No prose describing phantom parameters
- [ ] All action types documented in action.md
- [ ] Config page matches actual config system
- [ ] Delta page explains baseline model, not since_frame model

---

## Unit 8: Fix token budget documentation

**File**: `site/guide/token-budgets.md`

### Default detail level
```
Old: `summary` (default)
New: `standard` (default)
```
Fix line 21 heading and surrounding text.

### Default token budget
```
Old: default 2000
New: tier-based defaults — summary: 500, standard: 1500, full: 3000
```
Fix line 66 and the guidelines table.

### `spatial_delta` example
Remove `since_frame` from the example JSON on line 118. Update to explain the
baseline model.

### `spatial_config` example
```
Old: "default_token_budget": 3000, "default_detail": "summary"
New: "token_hard_cap": 3000 (or remove this section if config params are wrong)
```

**Acceptance Criteria**:
- [ ] Default detail documented as "standard"
- [ ] Default budgets documented as tier-based (500/1500/3000)
- [ ] No `since_frame` in examples
- [ ] Config examples match actual params

---

## Unit 9: Fix Director subpages (`site/director/*.md`)

Align prose and examples in Director guide pages with the corrected API
reference from Unit 6.

**Files to check/fix**:
- `site/director/scenes.md` — `path`→`scene_path`, `root_class`→`root_type`
- `site/director/nodes.md` — `scene`→`scene_path`, `node`→`node_path`, etc.
- `site/director/resources.md` — `path`→`resource_path`, `save_path`→`resource_path`
- `site/director/animation.md` — rewrite for resource_path model
- `site/director/physics.md` — restructure for 2 operations
- `site/director/wiring.md` — field name fixes
- `site/director/tilemaps.md` — field name fixes
- `site/director/batch.md` — verify
- `site/director/editor-backend.md` — fix port 6550→6551
- `site/director/daemon.md` — fix port 6551→6550
- `site/director/index.md` — fix port references

**Acceptance Criteria**:
- [ ] All examples use actual parameter names
- [ ] Port numbers are correct throughout
- [ ] Animation examples use resource_path model

---

## Unit 10: Fix error reference

**File**: `site/api/errors.md`

- Fix port references in backend error table
- All "Stage" references should already be correct (fixed in prior pass)
- Verify error messages match actual error strings in code

**Acceptance Criteria**:
- [ ] Port numbers correct
- [ ] Zero "stage" references

---

## Unit 11: Fix examples pages

**Files**: `site/examples/*.md`

Scan all example pages for:
- Incorrect parameter names in JSON examples
- References to `stage` binary
- Wrong port numbers

Fix any found.

**Acceptance Criteria**:
- [ ] Zero "stage" references
- [ ] JSON examples use correct parameter names

---

## Unit 12: Fix architecture pages

**Files**: `site/architecture/*.md`

- `site/architecture/index.md` — fix crate names, addon paths
- `site/architecture/crates.md` — fix crate names to `stage-*`
- `site/architecture/tcp.md` — fix port numbers, protocol section names
- `site/architecture/contributing.md` — fix build commands, crate names

**Acceptance Criteria**:
- [ ] Zero "stage" references
- [ ] Crate names are `stage-*`
- [ ] Port numbers correct

---

## Implementation Order

1. **Unit 1**: CLAUDE.md — foundational, affects all future agent sessions
2. **Unit 3**: docs/ directory — bulk find-replace, no code dependencies
3. **Unit 2**: site source files — remaining stage references
4. **Unit 4**: Director port numbers — critical factual fix
5. **Unit 5**: Stage API reference rewrite — largest unit
6. **Unit 6**: Director API reference rewrite — second largest
7. **Unit 7**: Stage tool subpages — align with new API ref
8. **Unit 8**: Token budget docs — align defaults
9. **Unit 9**: Director subpages — align with new API ref
10. **Unit 10-12**: Error ref, examples, architecture — mop-up

Units 1-4 can be parallelized. Units 5-6 can be parallelized. Units 7-12
can be parallelized after 5-6 complete.

---

## Testing

### Verification commands

```bash
# Zero stage references anywhere in the repo's docs
grep -rci stage CLAUDE.md site/ docs/ .claude/ \
  --include='*.md' --include='*.vue' --include='*.mts' \
  --include='*.ts' --include='*.css'
# Expected: 0 total matches

# Site builds without errors
cd site && npm run build

# Verify no broken internal links (VitePress reports dead links during build)
```

### Manual verification

- [ ] Read through the Stage API reference and spot-check 3 tools against
      their actual `*Params` struct definitions
- [ ] Read through the Director API reference and spot-check 3 operations
      against their actual params structs
- [ ] Verify port numbers in ArchDiagram render correctly
- [ ] Verify the home page renders with Stage branding (no Stage)

---

## Verification Checklist

```bash
# 1. Zero stage references
grep -rci stage CLAUDE.md site/ docs/ .claude/ --include='*.md' --include='*.vue' --include='*.mts' --include='*.ts' --include='*.css'

# 2. Site builds
cd site && npm run build

# 3. Correct port numbers
grep -rn '6550\|6551' site/ --include='*.md' --include='*.vue'
# Verify: editor=6551, daemon=6550

# 4. Correct binary name
grep -rn 'stage ' site/ --include='*.md' | grep -v 'Stage ' | head -20
# Should show CLI examples using `stage` binary

# 5. Correct crate names
grep -rn 'stage-server\|stage-godot\|stage-protocol\|stage-core' CLAUDE.md
# Should find the correct crate names
```
