# Changelog

All notable changes to Theatre are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Theatre uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Spectator
- `spatial_query` type `relationship` now returns `in_fov` field for Camera3D `from` nodes
- `recording` query conditions: added `velocity_above` and `property_equals` filter types
- `spatial_config`: added `auto_record` and `max_clip_duration_s` for continuous background recording
- Editor dock: clip list now shows duration and marker count inline
- `spatial_snapshot`: `include_properties` now supports dot-notation for nested properties

### Director
- `batch_execute`: added `stop_on_error: false` mode for partial application
- `animation_set_key`: added `easing` parameter with `linear`, `ease_in`, `ease_out`, `ease_in_out` options
- `scene_diff`: new operation to compare two scene files
- `gridmap_fill`: new operation for batch-filling 3D regions in GridMap
- Daemon backend: improved startup detection (no longer requires a 3-second sleep in CI)

### Infrastructure
- Codec: enforce 16MB maximum message size to prevent memory exhaustion
- E2E test harness: `DirectorFixture::new()` now supports one-shot and daemon modes
- All crates updated to Rust edition 2024

---

## [0.1.0] — 2026-03-12

Initial release of Theatre — an AI agent toolkit for Godot game engine.

### Spectator

**9 MCP tools for observing running Godot games:**

- `spatial_snapshot` — Instant spatial snapshot of all tracked nodes. Supports `detail` levels (summary/full/custom), `budget_tokens` for response size control, `focus_node` for priority ordering, and type filtering.

- `spatial_delta` — Changes-only response since a given frame. Token-efficient alternative to repeated snapshots. Supports `min_distance_change` threshold to filter physics noise.

- `spatial_query` — Geometric queries: `nearest`, `radius`, `area`, `raycast`, `path_distance`, `relationship`. Path distance uses the scene's NavigationRegion3D baked navmesh.

- `spatial_inspect` — Deep inspection of a single node. Returns all tracked properties, signal connections, children, and spatial context (nearby nodes, parent relationship).

- `spatial_watch` — Register nodes for continuous monitoring. Watched nodes appear in subsequent `spatial_delta` responses when their tracked properties change. Returns a stable `watch_id`.

- `spatial_config` — Configure tick rate, capture radius, tracked node types, and ring buffer depth. Changes take effect immediately.

- `spatial_action` — Set properties, call methods, or emit signals on running game nodes. For testing and hypothesis verification; changes are not persistent.

- `scene_tree` — Scene tree structure without spatial data. Compact and fast — good for orientation and path lookup.

- `recording` — Record gameplay to clip files, mark bug moments (F9), and query the spatial timeline. Supports condition filtering (`proximity`, `velocity_above`, `property_equals`) over frame ranges.

**GDExtension (spectator-godot):**
- Targets Godot 4.2+ with `compatibility_minimum = "4.2"`
- Uses `lazy-function-tables` for compatibility across 4.2–4.6+
- TCP listener on port 9077 (127.0.0.1 only)
- Ring buffer: 600 frames (~10s at 60Hz)
- Collection: O(n) over tracked nodes in `_physics_process`
- Graceful degradation: GDScript addon loads without crash if `.so` is missing

**Editor dock:**
- Recording controls (Start / Mark Bug / Stop)
- Keyboard shortcuts: F8 (record), F9 (mark), F10 (stop)
- Clip list with duration, frame count, and marker display
- Active watches display with delete controls
- Activity feed showing recent agent tool calls

### Director

**25+ operations across 8 domains:**

Scenes: `scene_create`, `scene_read`, `scene_list`, `scene_instance`, `scene_diff`

Nodes: `node_add`, `node_remove`, `node_set_property`, `node_get_property`, `node_move`, `node_rename`

Resources: `resource_create`, `resource_set`, `resource_get`, `resource_list`

TileMap/GridMap: `tilemap_set`, `tilemap_get`, `tilemap_fill`, `tilemap_clear`, `gridmap_set`, `gridmap_get`, `gridmap_fill`

Animation: `animation_create`, `animation_add_track`, `animation_set_key`, `animation_play`

Shaders: `shader_set`, `shader_get_param`, `shader_set_param`

Physics: `physics_layer_names`, `physics_layer_set`, `physics_mask_set`

Wiring: `signal_connect`, `signal_disconnect`, `signal_list`, `export_set`

Batch: `batch_execute`

**Three-backend routing:**
- Editor plugin backend (port 6550) — preferred when editor is open
- Headless daemon backend (port 6551) — fallback for CI/CD
- One-shot subprocess — always available, slower

**GDScript addon (addons/director/):**
- Pure GDScript — no GDExtension required
- Editor dock showing backend status and operation log
- Daemon script for headless operation

### Wire Protocol
- TCP length-prefixed JSON: 4-byte big-endian u32 + JSON payload
- Shared codec between server and GDExtension (no duplication)
- Handshake with version validation on connection
- 16MB maximum message size enforcement
- All ports bind to 127.0.0.1 only

### Testing
- Unit tests co-located with source (`#[cfg(test)] mod tests`)
- Integration test harness: `TestHarness` (mock TCP), `E2EHarness` (full stack)
- E2E journey tests in `tests/wire-tests/` and `tests/director-tests/`
- E2E tests marked `#[ignore = "requires Godot binary"]`

[Unreleased]: https://github.com/nathanielfernandes/theatre/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/nathanielfernandes/theatre/releases/tag/v0.1.0
