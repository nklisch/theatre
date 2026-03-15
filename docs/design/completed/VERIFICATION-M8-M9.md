# Verification Report: M8 (Recording Analysis) + M9 (2D Support)

## Status: FAIL

## Build & Tests

- Build: PASS (warnings only — unused `HandshakeInfo` fields in tcp.rs)
- Tests: PASS (50 passed, 0 failed)

---

## Design Compliance

### M8: Recording Analysis

- [x] `recording` tool — `snapshot_at` action (recording.rs:237-269)
- [x] `recording` tool — `query_range` action (recording.rs:271-317)
- [x] `recording` tool — `diff_frames` action (recording.rs:319-345)
- [x] `recording` tool — `find_event` action (recording.rs:347-383)
- [x] `query_range` conditions: `proximity`, `velocity_spike`, `property_change`, `state_transition`
- [ ] `query_range` conditions: `signal_emitted`, `entered_area` — **GAP: stubbed, never match**
- [x] `diff_frames`: position changes (old/new + delta), state changes, markers between frames
- [x] `find_event` event types: signal, property_change, collision, area_enter, area_exit, node_added, node_removed, marker, input
- [x] SQLite queries for frame range selection
- [x] MessagePack deserialization of frame data (`rmp_serde::from_slice`)
- [x] Spatial condition evaluation in Rust (proximity, velocity spike, property change)
- [x] Token budget enforcement on recording query results
- [x] System marker generation: velocity spike detection (`evaluate_velocity_spike`)
- [x] System marker generation: property threshold crossing (`evaluate_property_change`)
- [ ] System marker generation: collision event correlation — **GAP: not implemented**

### M9: 2D Support

- [x] Dimension detection in handshake (`SceneDimensions` enum, handshake.rs:6-12)
- [x] 2D coordinate output: `[x, y]` positions (collector.rs:319)
- [x] 2D coordinate output: single `rot` angle (collector.rs:320)
- [x] 2D coordinate output: `[x, y]` velocities (collector.rs:358-369)
- [x] 2D bearing system: 8-direction compass without elevation (bearing.rs:157-203)
- [x] 2D spatial index: `GridHash2D` grid hash in stage-core (index.rs:134-237)
- [ ] 2D frustum check: Camera2D viewport rectangle — **GAP: `include_offscreen` param ignored for 2D**
- [ ] 2D physics: `PhysicsRayQueryParameters2D` for raycasts — **GAP: only 3D raycast exists, fails in 2D scenes**
- [x] 2D transform output: `Transform2D` (origin + angle) (collector.rs:394-406)
- [x] 2D physics state: CharacterBody2D `on_floor` in snapshot (collector.rs:371-390); `on_floor`/`on_wall`/`on_ceiling` in inspect (collector.rs:692-725, InspectPhysics protocol)
- [x] Dimension-aware collection: Node2D vs Node3D APIs (collector.rs:186-214)
- [x] Dimension-aware bearing, indexing, delta computation (bearing.rs, index.rs, SpatialIndex enum)
- [x] Mixed scene support: per-entity position-length detection (snapshot.rs:227)
- [ ] 2D example project (`examples/2d-platformer-demo/`) — **GAP: directory does not exist**

---

## Code Quality

- [x] MCP tool handler pattern: `query_addon`, `serialize_params`, `deserialize_response`, `finalize_response`, `log_activity` — all followed
- [x] No `.unwrap()` in library code
- [x] No `println!` anywhere (stdout is clean)
- [x] Serde enum tagging: `#[serde(tag = "type")]` and `#[serde(rename_all = "snake_case")]` — correct
- [x] Test coverage: 12 tests covering all 4 analysis actions (snapshot_at, query_range, diff_frames, find_event)
- [x] 2D/3D branching in collector.rs: clean separation, no duplicate logic
- [x] Error layering: `McpError::invalid_params()` / `McpError::internal_error()` correctly applied

---

## Gaps (Action Required)

### 1. [M8 — design compliance] `signal_emitted` / `entered_area` conditions in `query_range` never match

- File: `crates/stage-server/src/recording_analysis.rs:436`
- Expected: When `condition_type` is `"signal_emitted"`, query the events table for signal events in the frame range at the target node; return `RangeMatch` hits. Same for `"entered_area"` → `area_enter` events.
- Actual: Returns `None` with comment `// handled via events table` — events table is never queried. These conditions always produce zero results.
- Fix: Add `evaluate_signal_emitted()` and `evaluate_entered_area()` functions that query the events table for matching events. The events table has `frame`, `node`, `event_type`, `data` columns. For `signal_emitted`: query `SELECT * FROM events WHERE event_type='signal' AND node=? AND frame BETWEEN ? AND ?`, optionally filtering by `condition.signal_name`. For `entered_area`: query `event_type='area_enter'`. Return `RangeMatch { frame, time_ms, node, annotation: "signal_emitted" / "area_enter" }` per match. Remove the `=> None` stub.

### 2. [M9 — design compliance] Collision event correlation for system markers not implemented

- File: `crates/stage-server/src/recording_analysis.rs` (no collision detection function exists)
- Expected: During `query_range` or post-hoc analysis, detect collision events and auto-generate system markers. M8 roadmap deliverable: "Collision event correlation".
- Actual: Velocity spike and property threshold marker generation are implemented, but no collision event logic exists.
- Fix: Add `evaluate_collision()` function that queries `events WHERE event_type='collision'` and returns `RangeMatch` with annotation. Wire into `evaluate_condition()` dispatch at line 426.

### 3. [M9 — design compliance] Camera2D viewport frustum culling not implemented

- File: `crates/stage-godot/src/collector.rs:240-261` (`should_collect_2d`)
- Expected: When `include_offscreen: false` and perspective is `camera`, 2D entities outside Camera2D's viewport rectangle are excluded. M9 roadmap: "2D frustum check: Camera2D viewport rectangle instead of Camera3D frustum." M9 exit criteria: "Camera2D viewport culling."
- Actual: `should_collect_2d` only filters by `class_filter` and `groups`. `include_offscreen` is never checked. All 2D entities are included regardless of viewport visibility.
- Fix: In `should_collect_2d`, if `!params.include_offscreen` and perspective is camera-based, extract Camera2D from scene tree. Use Camera2D's `get_canvas_transform()` and viewport size to compute screen-space bounds. Check if the node's global position falls within the viewport rect. Return `false` if outside. A helper `is_visible_to_camera_2d(node: &Gd<Node2D>, camera: &Gd<Camera2D>) -> bool` is the cleanest approach.

### 4. [M9 — design compliance] 2D raycast uses 3D physics, fails in pure 2D scenes

- File: `crates/stage-godot/src/collector.rs:1476-1527` (`fn raycast`)
- Expected: In 2D scenes, raycasts use `PhysicsRayQueryParameters2D` and `World2D`. M9 roadmap: "2D physics: `PhysicsRayQueryParameters2D` for raycasts." M9 exit criteria: "Raycasts use 2D physics."
- Actual: `raycast()` calls `get_world_3d()` unconditionally. In a pure 2D scene, `get_world_3d()` returns `None`, and the function returns `Err("No World3D — is this a 3D scene?")`. 2D raycasts are completely broken.
- Fix: Add a dimension parameter to `raycast()` (or a parallel `raycast_2d(from: Vector2, to: Vector2)` function). The 2D version uses `PhysicsServer2D::singleton()`, `get_world_2d()`, `intersect_ray()` with `PhysicsRayQueryParameters2D`. In `action_handler.rs`, branch on scene dimensions when dispatching raycast queries — use 2D or 3D version accordingly. Return `blocked_at: [x, y]` (2-element) in the 2D case.

### 5. [M9 — completeness] 2D example project missing

- File: `examples/2d-platformer-demo/` (directory does not exist)
- Expected: A Godot project with 2D platformer demonstrating spatial snapshot, bearing output, and Camera2D viewport culling. M9 roadmap: "2D example project (`examples/2d-platformer-demo/`)."
- Actual: `examples/` directory does not exist.
- Fix: Create a minimal Godot 4 project at `examples/2d-platformer-demo/` with: a TileMap floor, a CharacterBody2D player, 2–3 enemy CharacterBody2D nodes, Camera2D tracking the player, and the Stage addon enabled. Include a `README.md` with agent query examples showing 2D output format.

---

## Summary

| Milestone | Deliverables | Implemented | Gaps |
|-----------|-------------|-------------|------|
| M8 | 15 | 13 | 2 (signal/area conditions stubbed; collision markers missing) |
| M9 | 14 | 10 | 4 (Camera2D frustum; 2D raycast; collision markers; example project) |

Note: Gap #2 (collision event correlation) and Gap #4 (2D raycast) are the most impactful. Gap #3 (Camera2D frustum) affects snapshot correctness in 2D. Gaps #1 and #5 are functional gaps with lower user impact.
