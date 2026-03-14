# Theatre Documentation Audit Report

**Date**: 2026-03-13
**Scope**: All site docs, design docs, top-level docs, README

---

## Executive Summary

The site documentation (`site/`) is **extensively out of sync** with the codebase. Nearly every tool reference page has wrong parameter names, wrong response formats, missing features, or describes APIs that don't exist. The design docs (`docs/design/`) are mostly completed implementations and should be archived. Top-level docs have moderate staleness.

**By the numbers:**
- **13 site tool pages**: 11 have major errors, 2 have moderate errors
- **51 design docs**: 38 completed, 5 obsolete, 2 partially completed
- **10 top-level docs**: 4 stale, 6 current

---

## Part 1: Site Docs Discrepancies

### 1.1 Global Issues (affect every page)

| Issue | Details |
|-------|---------|
| **Repository URL inconsistency** | Cargo.toml: `theatre-godot/theatre`, site+README: `nathanielfernandes/theatre` |
| **Rust version understated** | Site says 1.75+, README says 1.80+, actual edition 2024 requires **1.85+** |
| **Director port numbers swapped everywhere** | Docs say editor=6550, daemon=6551. Code: editor=**6551**, daemon=**6550** |

### 1.2 Stage Tool Pages

#### `snapshot.md` — MAJOR REWRITE NEEDED
- `detail` default claimed as `"summary"`, actual is `"standard"`
- `include_properties` param doesn't exist
- Missing params: `perspective`, `focal_point`, `radius`, `groups`, `include_offscreen`, `expand`
- Response shows flat `nodes` map; actual uses `entities` array (standard/full) or `clusters` (summary)
- Response fields `node_count`, `included_nodes`, `truncated` don't exist; actual has `pagination` block
- Missing response sections: `perspective` block, `static_summary`, budget info

#### `delta.md` — MAJOR REWRITE NEEDED
- Claims `since_frame` param (required); **doesn't exist** — delta uses stored baseline from last snapshot
- `min_distance_change`, `min_velocity_change` params don't exist
- Missing params: `perspective`, `radius`, `groups`
- Response shows flat `nodes` map with `elapsed_ms`; actual returns categorized arrays: `moved`, `state_changed`, `entered`, `exited`, `watch_triggers`, `static_changed`, `signals_emitted`

#### `query.md` — MAJOR REWRITE NEEDED
- `k` default claimed as 10, actual is **5**
- Raycast: doc says `from`+`direction`, code uses `from`+`to`
- Area: doc says bounding-box with `min`/`max`, code treats area as radius alias
- `max_distance`, `collision_mask` MCP params don't exist
- Missing param: `groups`, `token_budget`
- Response field mismatches: `node` vs `path`, missing `bearing`, relationship fields completely different (`occluded`/`in_fov` don't exist; actual has `line_of_sight`/`elevation_diff`/`occluder`)
- `path_distance`: `reachable` should be `traversable`, `distance` should be `nav_distance`

#### `inspect.md` — MODERATE REWRITE
- Default include claimed as `["properties", "spatial_context"]`; `"properties"` is not a valid category
- Actual categories: `transform`, `physics`, `state`, `children`, `signals`, `script`, `spatial_context`, `resources`
- Response structure differs: separate `transform`/`physics`/`state` sections, not flat `properties` object
- `spatial_context` fields differ: actual has `nearby_entities`, `in_areas`, `camera_visible`, `camera_distance`

#### `watch.md` — MAJOR REWRITE NEEDED
- Action names wrong: `"create"` → `"add"`, `"delete"` → `"remove"`
- Params are flat (`node`, `track`); actual nests inside `watch` object
- `track` values are categories (`position`/`state`/`signals`/`physics`/`all`), not property names like `"health"`
- Missing: `conditions` array (the core conditional watch feature)

#### `config.md` — COMPLETE REWRITE NEEDED (entirely fictional)
- Every documented param is wrong: `tick_rate`, `capture_radius`, `tracked_types`, `buffer_depth_frames`, `default_token_budget`, `default_detail`, `record_path`, `capture_center`, `extra_tracked_types` — **none exist**
- Actual params: `static_patterns`, `state_properties`, `cluster_by`, `bearing_format`, `expose_internals`, `poll_interval`, `token_hard_cap`

#### `action.md` — MODERATE REWRITE
- Only documents 3 of 9 action types
- Missing: `pause`, `advance_frames`, `advance_time`, `teleport`, `spawn_node`, `remove_node`
- Missing param: `return_delta`
- `signal_args` should be `args`

#### `scene-tree.md` — MAJOR REWRITE NEEDED
- Missing critical `action` param (`roots`/`children`/`subtree`/`ancestors`/`find`)
- `root` param doesn't exist (uses `node`)
- `max_depth` should be `depth`, default 3 not 5
- `show_properties` should be `include` with enum values

#### `recording.md` — MAJOR REWRITE NEEDED
- Describes start/stop recording model; actual uses **dashcam** (always-on ring buffer)
- `start`, `stop` actions don't exist
- `mark` → `add_marker`, `query_frame` → `snapshot_at`
- Missing actions: `save`, `status`, `markers`, `trajectory`, `diff_frames`, `find_event`, `screenshot_at`, `screenshots`
- `query_range`: `start_frame`/`end_frame` → `from_frame`/`to_frame`, `nodes` (array) → `node` (single)
- Condition types differ: `velocity_above` → `velocity_spike`, `property_equals` → `property_change`

#### `dashcam.md` — MODERATE FIXES
- References `clips { "action": "start" }` and `clips { "action": "stop" }` — don't exist
- References `auto_record`, `max_clip_duration_s` config options — don't exist

#### `watch-workflow.md` — MODERATE FIXES
- Uses wrong action names (`create`/`delete` vs `add`/`remove`)
- Wrong param structure (flat vs nested `watch` object)
- Wrong track values (property names vs categories)

#### `editor-dock.md` — MINOR FIXES
- Mostly UI description, hard to verify against server code
- References start/stop recording buttons (dashcam model differs)

### 1.3 Director Tool Pages

#### `scenes.md` — MODERATE REWRITE
| Doc param | Actual param |
|-----------|-------------|
| `path` | `scene_path` |
| `root_class` | `root_type` |
| `root_name` | **doesn't exist** |
| `max_depth` | `depth` |
| `source_scene` | `instance_scene` |
| `name` | `node_name` |
| `scene` (instance) | `scene_path` |
| `parent` (instance) | `parent_path` |
| `path` (uid_get) | `file_path` |
| `scene`/`save_path` (mesh lib) | `scene_path`/`output_path` |
- Missing params: `scene_read.properties`, `scene_list.pattern`, `export_mesh_library.items`, `uid_update_project.directory`
- `scene_add_instance` claims `position` param — doesn't exist

#### `nodes.md` — MODERATE REWRITE
| Doc param | Actual param |
|-----------|-------------|
| `scene` | `scene_path` |
| `parent` | `parent_path` |
| `name` | `node_name` |
| `class` | `node_type` |
| `node` | `node_path` |
| `new_parent` | `new_parent_path` |
| `class` (find) | `class_name` |
| `script` | `script_path` |
- `node_add` claims `position` param — doesn't exist
- Missing `node_find` params: `property`, `property_value`, `limit`

#### `resources.md` — MODERATE REWRITE
| Doc param | Actual param |
|-----------|-------------|
| `path` | `resource_path` |
| `properties` (read filter) | **doesn't exist** — has `depth` instead |
| `save_path` (material) | `resource_path` |
| `properties` (shape) | `shape_params` |
| `save_path` (style_box) | `resource_path` |
- Missing params: `material_create.shader_path`, `shape_create.scene_path`/`node_path`, `resource_duplicate.property_overrides`/`deep_copy`

#### `tilemaps.md` — MODERATE REWRITE
- `cells[].position` → `cells[].coords`
- `cells[].layer` doesn't exist (TileMapLayer architecture)
- Region format: doc uses `min/max`, actual uses `position/size` (Rect2i)
- `tilemap_clear`: has `region` not `layer`
- `gridmap_get_cells`: `region` → `bounds`
- Missing params: `tilemap_get_cells.source_id`, `gridmap_get_cells.item`, `gridmap_clear.bounds`

#### `animation.md` — MAJOR REWRITE NEEDED
- Fundamental architecture mismatch: doc says AnimationPlayer-based (`node` + `animation_name`), code creates standalone `.tres` files (`resource_path`)
- `loop_mode` values differ: `"loop"` → `"linear"`, `"ping_pong"` → `"pingpong"`
- `track_type` values completely different: doc has `"property"`, code has `"value"`, `"position_3d"`, `"rotation_3d"`, etc.
- `easing` (named string) → `transition` (float)
- Missing tools: `animation_read`, `animation_remove_track` undocumented

#### `shaders.md` — MAJOR REWRITE NEEDED
- Omits entire node graph definition (`nodes` array, `connections`, `shader_function`)
- `save_path` → `resource_path`
- Description is extremely simplified vs actual capability

#### `physics.md` — MAJOR REWRITE NEEDED
- Tool names wrong: `physics_layer_names` → `physics_set_layer_names`, `physics_layer_set` → `physics_set_layers`, `physics_mask_set` → **doesn't exist** (merged into `physics_set_layers`)
- `physics_set_layer_names` missing required `layer_type` param
- No getter for layer names (doc claims dual get/set)
- Array-of-layer-numbers format doesn't exist — only raw bitmask integers

#### `wiring.md` — MODERATE REWRITE
| Doc param | Actual param |
|-----------|-------------|
| `from_node` | `source_path` |
| `signal` | `signal_name` |
| `to_node` | `target_path` |
| `method` | `method_name` |
| `node` (list) | `node_path` |
- Signal flag `1` described as one-shot; actual one-shot is flag `4`

#### `batch.md` — CORRECT
- Parameters match actual `BatchParams` struct

#### `editor-backend.md` — MODERATE FIXES
- Port wrong (says 6550, actual 6551)
- Project settings path wrong
- Claims Director dock panel exists — it doesn't
- Claims EditorUndoRedoManager integration — likely aspirational

#### `daemon.md` — MODERATE FIXES
- Port wrong (says 6551, actual 6550)
- `start-daemon.sh` doesn't exist
- `director batch-run`/`director run-stdin` subcommands don't exist
- Connection timeout: doc says 200ms, actual is 2 seconds

### 1.4 Guide / Architecture / API Pages

#### `architecture/crates.md` — MODERATE FIXES
- File structures wrong: `tools/` → `mcp/`, `session.rs` → `tcp.rs`, `spatial.rs` → doesn't exist, `diff.rs` → `delta.rs`
- Claims director has no stage-protocol dependency — **it does**
- Omits many stage-protocol files

#### `architecture/tcp.md` — MODERATE REWRITE
- Codec function signatures wrong: shows `&[u8]` API, actual is generic `Serialize`/`DeserializeOwned`
- `CodecError` variants wrong: missing `Serialize`/`Deserialize`, has non-existent `ConnectionClosed`
- Claims `thiserror` — actual uses manual `impl Display`/`impl Error`
- Handshake fields wrong: `version` → `stage_version`, missing `protocol_version`/`scene_dimensions`/`physics_ticks_per_sec`

#### `api/index.md` — MAJOR REWRITE
- All Stage tool param/response details inherit errors from individual tool pages
- `spatial_config` params entirely fictional
- `clips` actions wrong (start/stop model)
- Default detail "summary" wrong (actual "standard")

#### `api/director.md` — MODERATE REWRITE
- Physics tool names wrong
- Missing `animation_read`, `animation_remove_track`
- Changelog says `scene_instance`, actual is `scene_add_instance`

#### `api/wire-format.md` — MODERATE REWRITE
- Handshake fields wrong
- Request type naming may not match actual enum variants

#### `guide/installation.md` — MINOR FIXES
- Rust version wrong (1.75+ → 1.85+)

#### `guide/*` (other guide pages) — MINOR-MODERATE
- Inherit wrong tool param examples from reference pages
- Architecture descriptions use wrong file names

---

## Part 2: Design Docs Status

### Ready to Archive (COMPLETED)

| Doc | Evidence |
|-----|----------|
| M0-SKELETON.md | All 4 crates exist, TCP handshake works |
| M1-SNAPSHOT.md | spatial_snapshot, bearings, budget, clustering, R-tree all implemented |
| M2-INSPECT-SCENE-TREE.md | spatial_inspect + scene_tree handlers, wire tests |
| M3-ACTIONS-QUERIES.md | spatial_action + spatial_query handlers, wire tests |
| M4-DELTAS-WATCHES.md | spatial_delta + spatial_watch handlers, engines in core |
| M8-RECORDING-ANALYSIS.md | clip_analysis.rs with all query functions, SQLite, ClipSession |
| M9-2D-SUPPORT.md | SceneDimensions, Position2, 2D bearing, GridIndex2D, E2E test |
| M10-RESOURCE-INSPECTION.md | InspectCategory::Resources, meshes/sprites/etc fields |
| M11-DASHCAM.md | Ring buffer, clip state machine, merge logic, rate limiting |
| M11-DASHCAM-TESTS.md | Unit + integration + E2E journey tests |
| DIRECTOR-P1-MVP.md | scene_create, scene_read, node_add/set/remove, backend.rs |
| phase2-scene-composition.md | scene_add_instance, node_reparent, scene_list, resource_read |
| phase3-headless-daemon.md | daemon.rs, daemon.gd, backend selection |
| director-phase4-resources.md | material/shape/stylebox create, resource_duplicate |
| director-phase5-tilemap-gridmap.md | tilemap/gridmap set/get/clear |
| phase6-animation.md | animation create/add_track/read/remove_track |
| phase7-editor-plugin.md | editor.rs, editor_ops.gd, plugin.gd |
| phase-8-advanced-tools.md | visual_shader_create, physics tools |
| director-phase9-meta-operations.md | batch, scene_diff, uid_get/update, mesh_library |
| director-phase10-wiring-deferred.md | signal connect/disconnect/list, groups/script/meta/find |
| REFACTOR-M1-M2.md | Helpers extracted, integrated |
| REFACTOR-M4-M6.md | Shared config helpers, defaults.rs |
| REFACTOR-M6-M9.md | ParseMcpEnum trait, ClipSession pattern |
| REFACTOR-MCP-HANDLERS.md | defaults.rs, director_tool! macro |
| refactor-plan.md | SQLite helpers, contract fixes applied |
| refactor-plan-workspace.md | Shared async TCP codec |
| refactor-workspace.md | dist → distance contract fix |
| refactor-workspace-2.md | director_tool! macro extraction |
| INTEGRATION-TESTS.md | TCP mock tests, scenarios, E2E journeys |
| GDEXT-TESTING.md | Wire tests against real Godot headless |
| E2E-JOURNEY-TESTS.md | 6 Stage journey tests |
| director-journey-e2e-tests.md | 20+ Director journey tests |
| VERIFICATION-M6-M8.md | Historical verification report |
| VERIFICATION-M8-M9.md | Historical verification report |
| CLIP-SCREENSHOTS.md | screenshot_at/screenshots actions, JPEG, SQLite blobs |
| DASHCAM-ONLY-REFACTOR.md | recording → clips rename, dashcam-only model |
| MCP-TOOL-UX.md | Tool description improvements applied |
| theatre-rename.md | Rename completed |
| github-pages.md | VitePress site deployed |

### Partially Completed (keep in docs/design/)

| Doc | What's Missing |
|-----|---------------|
| M5-CONFIGURATION.md | Keybinding settings; ROADMAP checkboxes stale but code exists |
| M6-EDITOR-DOCK.md | Uses EngineDebugger not TCP events; session info section partial |

### Top-Level Docs Needing Updates

| Doc | Issue |
|-----|-------|
| SPEC.md | Tool #9 called `recording`, should be `clips` |
| TECH.md | Layout header updated to `theatre/ |
| CONTRACT.md | Tool #9 `recording` with start/stop/status — all removed |
| ROADMAP.md | Many checkboxes unchecked despite features being implemented |
| UX.md | References `recording` tool, start/stop controls |

---

## Part 3: Fix Plan

### Phase A: Archive Completed Designs

1. Create `docs/design/completed/` directory
2. Move all 38 COMPLETED design docs to `docs/design/completed/`
3. Keep M5, M6 in `docs/design/` (partially completed)
4. Add note to CLAUDE.md about completed docs

### Phase B: Fix Top-Level Docs (quick wins)

1. **SPEC.md**: `recording` → `clips`, update action list
2. **TECH.md**: `theatre/` in layout header (updated)
3. **CONTRACT.md**: Update tool #9 contract for clips/dashcam model
4. **ROADMAP.md**: Update checkboxes to reflect actual implementation status
5. **UX.md**: Update recording references to clips/dashcam

### Phase C: Fix Site — Global Issues

1. Standardize repository URL across Cargo.toml and site
2. Fix Rust minimum version to 1.85+ in installation.md, contributing.md, README.md
3. Fix Director port numbers everywhere (editor=6551, daemon=6550)

### Phase D: Fix Site — Stage Tool Pages (highest priority)

Rewrite these from code as source of truth:

1. **config.md** — complete rewrite (entirely fictional)
2. **snapshot.md** — rewrite params, response format, add perspective system
3. **delta.md** — rewrite params (remove since_frame, explain baseline model), rewrite response
4. **query.md** — fix k default, raycast model, area model, response fields
5. **watch.md** — fix action names, param structure, track model, add conditions
6. **recording.md** — rewrite for dashcam model, fix all action names, add missing actions
7. **scene-tree.md** — add action param, fix param names
8. **action.md** — document all 9 action types
9. **inspect.md** — fix include categories, response structure
10. **dashcam.md** — remove start/stop references, fix config references
11. **watch-workflow.md** — fix action names and param structure
12. **index.md** — fix config description, inspect include example

### Phase E: Fix Site — Director Tool Pages

Fix parameter names (systematic `scene`→`scene_path`, `node`→`node_path`, etc.) and structural issues:

1. **animation.md** — major rewrite (AnimationPlayer → resource file model, track types, easing)
2. **physics.md** — major rewrite (tool names, merged layer/mask, bitmask format)
3. **shaders.md** — major rewrite (add node graph definition)
4. **scenes.md** — fix all param names, remove fictional `root_name`/`position`
5. **nodes.md** — fix all param names, remove fictional `position`, add missing find params
6. **resources.md** — fix param names, `properties`→`depth`, add missing params
7. **tilemaps.md** — fix coords/region format, TileMapLayer model, missing params
8. **wiring.md** — fix param names, fix signal flag values
9. **editor-backend.md** — fix port, remove fictional dock, fix settings path
10. **daemon.md** — fix port, remove fictional scripts/subcommands, fix timeout

### Phase F: Fix Site — Architecture & API Pages

1. **architecture/crates.md** — fix file structures, add director→stage-protocol dep
2. **architecture/tcp.md** — fix codec signatures, CodecError, handshake fields
3. **api/index.md** — regenerate from corrected tool pages
4. **api/director.md** — fix tool names, add missing tools
5. **api/wire-format.md** — fix handshake fields
6. **guide/installation.md** — fix Rust version

### Phase G: CLAUDE.md Update

Add warning about completed design docs not being ground truth.

---

## Priority Order

1. **Phase A** (archive) + **Phase G** (CLAUDE.md) — prevent future confusion
2. **Phase C** (global fixes) — low effort, high impact
3. **Phase D** (Stage tool pages) — most user-facing, most broken
4. **Phase E** (Director tool pages) — systematic param name fixes
5. **Phase B** (top-level docs) — internal reference
6. **Phase F** (architecture/API) — secondary reference pages
