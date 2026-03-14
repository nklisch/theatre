# Spectator — Roadmap

This roadmap translates the full design (VISION, SPEC, CONTRACT, UX, USER_STORIES) into a phased implementation plan. Each milestone is a shippable state — usable, testable, and valuable on its own. Later milestones build on earlier ones without requiring rework.

The design docs describe the **complete system**. This roadmap describes **the order we build it**.

---

## Milestone 0: Skeleton ✅

**Goal:** Repo structure exists, both Rust artifacts compile, GDExtension loads in Godot, TCP handshake works. Nothing useful yet — just proof that the plumbing connects.

**Status: Complete**

### Deliverables

- [x] Cargo workspace with 4 crates (`spectator-server`, `spectator-godot`, `spectator-protocol`, `spectator-core`)
- [x] `spectator-server` compiles as a binary, starts tokio runtime, prints "waiting for connection"
- [x] `spectator-godot` compiles as cdylib, loads in Godot 4.5+ without errors
- [x] GDExtension manifest (`.gdextension`) with at least Linux debug target
- [x] GDScript `plugin.gd` (EditorPlugin) — enables/disables cleanly, registers autoload
- [x] GDScript `runtime.gd` (autoload) — instantiates GDExtension `SpectatorTCPServer` node
- [x] `SpectatorTCPServer` listens on configurable port (default 9077, localhost only)
- [x] `spectator-server` connects to addon via TCP
- [x] Handshake exchange: addon sends version/dimensions/project, server ACKs
- [x] `spectator-protocol` crate defines handshake message types with serde
- [x] Length-prefixed JSON codec (4-byte big-endian + JSON) working in both directions
- [x] Basic reconnection: server retries every 2s when connection drops
- [x] `plugin.cfg` with metadata
- [x] `CLAUDE.md` with repo conventions for agents working in this codebase
- [x] CI: `cargo fmt --check`, `cargo clippy --workspace`, `cargo test --workspace`, `cargo build --release`

### Exit Criteria

Run Godot with the addon enabled → hit Play → `spectator-server` connects → handshake logged on both sides → stop game → server reconnects when game restarts.

### Notes

- GDExtension built with `api-4-5` + `lazy-function-tables` (godot-rust). Requires Godot 4.5+; `lazy-function-tables` provides forward compatibility with 4.6+. `lazy-function-tables` defers method hash validation to first call, so hash changes in unused Godot APIs don't panic at init.
- `runtime.gd` uses `ClassDB.instantiate(&"ClassName")` instead of `ClassName.new()` and untyped vars for GDExtension types. Prevents GDScript parse errors if the extension fails to load.
- `theatre-deploy` script (`~/.local/bin/theatre-deploy`) handles build + copy to installed Godot projects in one command.

---

## Milestone 1: First Useful Tool

**Goal:** An agent can call `spatial_snapshot` and get real data back from a running Godot game. The core loop works end-to-end.

**Depends on:** M0

### Deliverables

- [ ] `SpectatorCollector` GDExtension class — traverses scene tree, collects node data
  - [ ] `get_visible_nodes()` — returns nodes in camera frustum/viewport
  - [ ] `get_node_transform(path)` — position, rotation, velocity, physics state
  - [ ] `get_node_state(path)` — exported variables
  - [ ] `get_frame_info()` — current frame, delta, timestamp
  - [ ] `get_dimensions()` — 2D or 3D
- [ ] TCP query/response flow: server sends query → addon dispatches to collector → returns data
- [ ] `spectator-core`: bearing calculation (8-direction cardinal + degrees + elevation)
- [ ] `spectator-core`: token budget estimation (`json_bytes / 4`)
- [ ] MCP tool registration via rmcp: `spatial_snapshot` with parameters
- [ ] `spatial_snapshot` detail tiers:
  - [ ] `summary` — clusters by group, nearest/farthest, counts
  - [ ] `standard` — per-entity: path, class, rel (dist/bearing/elevation/occluded), abs position, rotation, velocity, groups, state, recent signals
  - [ ] `full` — adds: full transform, physics, children, script, all exported vars, static node listing
- [ ] Perspective options: `camera`, `node`, `point`
- [ ] Filters: `groups`, `class_filter`, `radius`, `include_offscreen`
- [ ] Token budget enforcement with truncation (nearest-first ordering)
- [ ] Pagination: `cursor` parameter, `pagination` block in response
- [ ] Cluster expansion: `expand` parameter
- [ ] `budget` block on every response
- [ ] Static node detection (class-based heuristic: StaticBody3D, etc.)
- [ ] Static node caching (collected once, summarized in standard, listed in full)
- [ ] Error responses: `not_connected`, `scene_not_loaded`, `node_not_found`, `timeout`
- [ ] MCP config: `.mcp.json` example for Claude Code

### Exit Criteria

Agent calls `spatial_snapshot(detail: "summary")` → gets clustered overview of a running 3D Godot scene with correct bearings, distances, and groups. Agent drills into a cluster with `expand`. Agent gets `standard` detail with per-entity data. Token budget is respected. Pagination works for large scenes.

---

## Milestone 2: Inspect & Scene Tree

**Goal:** Agent can deeply inspect individual nodes and navigate the scene hierarchy. These two tools are high-value and independent of the delta/watch/recording systems.

**Depends on:** M1

### Deliverables

- [ ] `spatial_inspect` tool — full implementation
  - [ ] All `include` categories: transform, physics, state, children, signals, script, spatial_context
  - [ ] Selective inclusion (only requested categories)
  - [ ] `spatial_context`: auto-populated nearby entities, areas, navmesh edge distance, camera visibility
  - [ ] Signal connection map (connected signals with targets)
  - [ ] Recent signal emissions
  - [ ] Script info (path, base class, methods, extends chain)
- [ ] `scene_tree` tool — full implementation
  - [ ] `roots` — top-level nodes
  - [ ] `children` — immediate children of a node
  - [ ] `subtree` — recursive tree with depth limit
  - [ ] `ancestors` — parent chain to root
  - [ ] `find` — search by name, class, group, script
  - [ ] Configurable `include` per node (class, groups, script, visible, process_mode)
- [ ] GDExtension additions:
  - [ ] `get_children(path)`, `get_ancestors(path)`, `find_nodes(by, value)`
  - [ ] `get_signals(path)` — connected signals + recent emissions
  - [ ] `get_scene_tree(depth, root, include)`

### Exit Criteria

Agent calls `spatial_inspect(node: "enemies/scout_02")` → gets transform, physics, state, children, signals, script, spatial_context. Agent calls `scene_tree(action: "find", find_by: "class", find_value: "CharacterBody3D")` → gets all matching nodes. Agent navigates hierarchy with subtree/children/ancestors.

---

## Milestone 3: Actions & Queries

**Goal:** Agent can manipulate game state for debugging and ask targeted spatial questions. The agent becomes an active debugger, not just an observer.

**Depends on:** M1

### Deliverables

- [ ] `spatial_action` tool — all action types:
  - [ ] `pause` / unpause scene tree
  - [ ] `advance_frames` — step N physics frames while paused
  - [ ] `advance_time` — step N seconds
  - [ ] `teleport` — move node to position (+ optional rotation)
  - [ ] `set_property` — change exported variable or built-in property
  - [ ] `call_method` — call method with args, return result
  - [ ] `emit_signal` — emit signal with args
  - [ ] `spawn_node` — instantiate scene at position
  - [ ] `remove_node` — queue_free
  - [ ] `return_delta` flag (requires M4 delta, but action response shape established here)
- [ ] `spatial_query` tool — all query types:
  - [ ] `nearest` — K nearest nodes to point/node
  - [ ] `radius` — all nodes within radius
  - [ ] `raycast` — line-of-sight / collision check
  - [ ] `relationship` — mutual spatial relationship between two nodes
  - [ ] `path_distance` — navmesh distance between two nodes
  - [ ] `area` — nodes within AABB or sphere
- [ ] GDExtension additions:
  - [ ] `raycast(from, to, mask)` — physics raycast (3D and 2D)
  - [ ] `get_nav_path(from, to)` — navigation path
  - [ ] `teleport_node(path, position, rotation)`
  - [ ] `set_node_property(path, property, value)`
  - [ ] `call_node_method(path, method, args)`
  - [ ] `emit_node_signal(path, signal, args)`
  - [ ] `pause_tree(paused)`
  - [ ] `advance_physics(frames)`
  - [ ] `spawn_node(scene_path, parent, name, position)`
  - [ ] `remove_node(path)`
- [ ] `spectator-core`: spatial index (rstar R-tree for 3D) for efficient nearest/radius queries
- [ ] Error responses: `method_not_found`, `eval_error`, `dimension_mismatch`

### Exit Criteria

Agent pauses game, teleports enemy to wall, advances 5 frames, takes a snapshot — sees enemy stopped at wall. Agent raycasts from enemy to player, gets obstruction info. Agent queries nearest 5 nodes to player with group filter. Agent calls `take_damage(50)` on a node and sees the result.

---

## Milestone 4: Deltas & Watches

**Goal:** Agent can track changes over time. The delta engine and watch system make iterative debugging possible — act, check, act, check.

**Depends on:** M1, M3 (for `return_delta`)

### Deliverables

- [ ] `spatial_delta` tool — full implementation
  - [ ] Diff against last query (automatic) or specific `since_frame`
  - [ ] Change categories: moved, state_changed, entered, exited, signals_emitted
  - [ ] `static_changed` flag
  - [ ] Same perspective/radius/filter options as snapshot
  - [ ] Token budget with truncation
- [ ] Delta engine in `spectator-core`:
  - [ ] Last-snapshot state storage
  - [ ] Per-entity history tracking
  - [ ] Change detection thresholds (position < 0.01, rotation < 0.1°, float < 0.001)
  - [ ] Event buffer for signals, node enters/exits
- [ ] `spatial_watch` tool — full implementation
  - [ ] `add` — subscribe to node or group with optional conditions
  - [ ] `remove` — unsubscribe by watch_id
  - [ ] `list` — show active watches
  - [ ] `clear` — remove all watches
  - [ ] Condition operators: lt, gt, eq, changed
  - [ ] Group watches (`group:enemies`)
  - [ ] Watch triggers in delta responses (`watch_triggers` block)
- [ ] GDExtension additions:
  - [ ] `subscribe_signal(path, signal)` / `unsubscribe_signal(path, signal)`
  - [ ] Push events from addon to server for signal emissions
- [ ] `return_delta` on `spatial_action` responses (wiring from M3)
- [ ] Watch persistence across reconnection (server re-sends on reconnect)

### Exit Criteria

Agent sets up watches on enemy group, advances game time, calls `spatial_delta()` — sees movement, state changes, and watch triggers. Agent's watch fires when enemy health drops below 20. `return_delta` on teleport shows immediate spatial consequences.

---

## Milestone 5: Configuration (partial)

**Goal:** Agent and human can configure Spectator's behavior — what to track, how to display, token limits. Three configuration surfaces with clear precedence.

**Depends on:** M1

### Deliverables

- [ ] `spatial_config` MCP tool — full implementation
  - [ ] `static_patterns` — glob patterns for static node classification
  - [ ] `state_properties` — per-group/class property tracking config
  - [ ] `cluster_by` — group, class, proximity, none
  - [ ] `bearing_format` — cardinal, degrees, both
  - [ ] `expose_internals` — include non-exported vars
  - [ ] `poll_interval` — collection frequency
  - [ ] `token_hard_cap` — server-enforced ceiling
- [x] Project Settings integration (Godot editor):
  - [x] `spectator/connection/port`
  - [x] `spectator/connection/auto_start`
  - [x] `spectator/recording/storage_path`, `max_frames`, `capture_interval`
  - [x] `spectator/display/show_agent_notifications`, `show_recording_indicator`
  - [ ] `spectator/keybindings/toggle_recording`, `drop_marker`, `toggle_pause`
  - [x] `spectator/tracking/default_static_patterns`, `token_hard_cap`
- [ ] `spectator.toml` file support (project root, version-controllable)
- [ ] Config precedence: `spatial_config` (session) > `spectator.toml` (project) > Project Settings (machine)
- [ ] Static node classification using configured patterns + heuristics + observation

### Exit Criteria

Agent calls `spatial_config(static_patterns: ["walls/*"], state_properties: { enemies: ["health"] })` — subsequent snapshots correctly classify walls as static and include only health in enemy state. Human sets port to 9078 in Project Settings — addon listens on 9078. `spectator.toml` overrides Project Settings.

---

## Milestone 6: Editor Dock (partial)

**Goal:** Human has a native Godot editor dock showing connection status, session info, and agent activity. The human side of the collaboration surface.

**Depends on:** M1, M4 (for watch display)

### Deliverables

- [x] `dock.tscn` — editor dock panel scene
- [x] `dock.gd` — dock panel script
- [ ] Connection status indicator (green/yellow/red + text)
- [ ] Session info section:
  - [ ] Nodes tracked (count + groups)
  - [ ] Active watches (count, from agent)
  - [ ] Current frame number
  - [ ] FPS display
- [ ] Agent Activity Feed:
  - [ ] Scrolling log of agent queries and actions
  - [ ] Color-coded: queries (gray), watches (blue), actions (yellow)
  - [ ] Auto-scroll, max 20 entries
  - [ ] Collapsible
- [x] Activity feed protocol:
  - [x] Server sends `activity_log` push events to addon via TCP
  - [x] Addon routes to dock for display
- [x] Agent action notifications in-game:
  - [x] Toast popups on CanvasLayer for `spatial_action` operations
  - [x] Auto-dismiss after 3 seconds
  - [x] Stackable (max 3 visible)
  - [x] Disableable in Project Settings

### Exit Criteria

Human enables addon, sees "Waiting..." in dock. Agent connects, dock shows "Connected". Agent takes actions — dock Activity Feed shows entries. Agent teleports a node — toast notification appears in game viewport. Human can follow along with what the agent is doing.

---

## Milestone 7: Recording — Capture (partial)

**Goal:** Human can record spatial timelines. Agent can start/stop recordings. Frame data is captured and stored in SQLite. Markers work from all three sources (human, agent, system).

**Depends on:** M1, M6 (for dock recording controls)

### Deliverables

- [x] `SpectatorRecorder` GDExtension class:
  - [ ] Frame capture in `_physics_process` (configurable interval)
  - [ ] Tracked node snapshots to in-memory ring buffer
  - [ ] MessagePack serialization of frame data
  - [ ] Signal emission capture
  - [ ] Input event capture (optional)
  - [ ] Max frames safety valve
- [ ] SQLite recording storage:
  - [ ] Schema: recording, frames, events, markers tables
  - [ ] WAL mode for non-blocking reads during writes
  - [ ] Periodic flush (every 60 frames) for crash safety
  - [ ] Storage in `user://spectator_recordings/`
- [ ] `recording` tool — capture actions:
  - [ ] `start` — begin recording with name and capture config
  - [ ] `stop` — finalize recording, return metadata
  - [ ] `status` — check if recording, frame count, duration
  - [ ] `list` — list saved recordings
  - [ ] `delete` — remove a recording
- [x] Markers:
  - [x] Human markers via F9 / dock button (with optional text note)
  - [ ] Agent markers via `recording(action: "add_marker")`
  - [ ] System markers: velocity spike detection, auto-generated on anomalies
  - [ ] `recording(action: "markers")` — list all markers
- [ ] Dock recording controls:
  - [ ] Record button (starts recording, prompts for name)
  - [ ] Stop button
  - [ ] Marker button (with text input)
  - [ ] Timer display (elapsed time, frame count, buffer size)
  - [ ] Recording library list (name, duration, date, delete button)
- [x] Keyboard shortcuts:
  - [x] F8 — toggle recording (via `runtime.gd` `_shortcut_input`)
  - [x] F9 — drop marker
  - [x] F10 — toggle pause
  - [x] Visual feedback: recording indicator (red dot), marker flash, pause overlay
  - [ ] Remappable via Project Settings

### Exit Criteria

Human presses F8, plays game for 10 seconds, presses F9 at bug moment, presses F8 to stop. Recording appears in dock library and via `recording(action: "list")`. Agent can also start/stop recordings via MCP. Markers from all three sources are stored. Partial recording survives a game crash (up to last flush).

---

## Milestone 8: Recording — Analysis

**Goal:** Agent can scrub through recorded timelines, query across frame ranges, diff frames, and search for events. The full collaborative debugging workflow is complete.

**Depends on:** M7

### Deliverables

- [ ] `recording` tool — analysis actions:
  - [ ] `snapshot_at` — spatial state at a specific frame/time (same shape as `spatial_snapshot`)
  - [ ] `query_range` — search across frame range with conditions
    - [ ] Condition types: proximity, property_change, signal_emitted, entered_area, velocity_spike, state_transition
    - [ ] Result annotations: first_breach, deepest_penetration
  - [ ] `diff_frames` — compare spatial state between two frames
    - [ ] Position changes (old/new + delta)
    - [ ] State changes (old/new)
    - [ ] Markers between frames
  - [ ] `find_event` — search timeline for event types
    - [ ] Event types: signal, property_change, collision, area_enter, area_exit, node_added, node_removed, marker, input
- [ ] Recording analysis engine in `spectator-server`:
  - [ ] SQLite queries for frame range selection
  - [ ] MessagePack deserialization of frame data
  - [ ] Spatial condition evaluation in Rust (proximity, velocity spike detection)
  - [ ] Token budget enforcement on recording query results
- [ ] System marker generation:
  - [ ] Velocity spike detection (threshold-based, post-hoc analysis)
  - [ ] Property threshold crossing detection
  - [ ] Collision event correlation

### Exit Criteria

The full workflow from UX.md works: human records, marks a bug with F9, stops recording. Agent queries markers, snapshots the frame before the bug, runs a proximity query to find when the enemy breached the wall, diffs the before/after frames, adds its own marker with the root cause. Agent reports findings to the human with frame references.

---

## Milestone 9: 2D Support

**Goal:** Spectator works correctly with 2D Godot games. All tools produce 2D-appropriate output.

**Depends on:** M1-M8 (layered on top of working 3D implementation)

### Deliverables

- [ ] Dimension detection in handshake (`scene_dimensions: 2`)
- [ ] 2D coordinate output: `[x, y]` positions, single `rot` angle, `[x, y]` velocities
- [ ] 2D bearing system: 8-direction compass without elevation
- [ ] 2D spatial index: grid hash in `spectator-core`
- [ ] 2D frustum check: Camera2D viewport rectangle instead of Camera3D frustum
- [ ] 2D physics: `PhysicsRayQueryParameters2D` for raycasts
- [ ] 2D transform output: `Transform2D` (origin + angle)
- [ ] 2D physics state: CharacterBody2D fields (on_floor, on_wall, on_ceiling)
- [ ] `spectator-godot`: dimension-aware collection (Node2D vs Node3D APIs)
- [ ] `spectator-core`: dimension-aware bearing, indexing, delta computation
- [ ] Mixed scene support (`scene_dimensions: "mixed"`)
- [ ] 2D example project (`examples/2d-platformer-demo/`)

### Exit Criteria

Agent calls `spatial_snapshot` in a 2D platformer → gets `[x, y]` positions, correct bearings (no elevation), Camera2D viewport culling. All 9 tools work correctly in 2D. Raycasts use 2D physics. Recordings capture 2D state correctly.

---

## Milestone 10: Resource Inspection

**Goal:** Agent can inspect loaded resources on nodes — meshes, materials, animations, collision shapes, shader parameters. Completes the deep inspection capability.

**Depends on:** M2

### Deliverables

- [ ] `resources` include option on `spatial_inspect`
- [ ] GDExtension: `get_node_resources(path)` — collects resource data
  - [ ] Mesh: resource path, type, surface count
  - [ ] Material overrides: per-surface material info
  - [ ] Collision shape: type, dimensions, inline vs file resource
  - [ ] AnimationPlayer: current animation, available anims, position, length, looping
  - [ ] NavigationAgent: map, target position, postprocessing
  - [ ] Shader parameters: exported shader uniforms with current values
- [ ] Inline vs file resource distinction

### Exit Criteria

Agent calls `spatial_inspect(node: "enemies/scout_02", include: ["resources"])` → sees mesh info, collision shape dimensions, current animation state, shader parameters. Agent can diagnose "why is this invisible" (no material), "why is it T-posing" (no animation), "what's the collider shape" problems.

---

## Milestone 11: Dashcam Recording

**Goal:** Always-on dashcam recording: the addon maintains a rolling in-memory ring buffer from the moment Godot loads the extension. When a marker fires — from game code, an agent, or a human — the system saves a clip covering the pre-window and post-window around the trigger. Each clip is a self-contained SQLite file, identical in schema to M7 explicit recordings.

**Depends on:** M7, M8

### Deliverables

- [ ] Ring buffer in `SpectatorRecorder` — always-on dashcam mode alongside explicit recording
- [ ] Clip state machine: Buffering → PostCapture → flush to SQLite
- [ ] Per-tier window configuration (system vs agent/human triggers)
- [ ] Merge policy: overlapping triggers produce one merged clip, not multiple
- [ ] System marker rate limiting (prevent noisy game code from accumulating unbounded clips)
- [ ] New GDExtension exports: `set_dashcam_enabled`, `is_dashcam_active`, `flush_dashcam_clip`
- [ ] New signals: `dashcam_clip_saved`, `dashcam_clip_started`
- [ ] `recording(action: "dashcam_status")` and `recording(action: "flush_dashcam")` MCP actions
- [ ] New TCP methods: `dashcam_status`, `dashcam_flush`, `dashcam_config`
- [ ] Configuration: `spectator.toml` dashcam keys + `spatial_config` session overrides
- [ ] Memory model with byte cap and adaptive pre-window
- [ ] Dock integration: dashcam status line, F9 in dashcam-only mode

### Exit Criteria

Addon starts buffering automatically with no MCP or human interaction. Game code calls `SpectatorRecorder.add_marker("system", "player_died")` from GDScript; a clip is saved covering the configured pre- and post-window around that frame. Agent calls `recording(action: "add_marker")` or human presses F9; clips save with appropriate tier windows. Overlapping triggers produce one merged clip. `recording(action: "list")` returns dashcam clips alongside explicit recordings. All M8 analysis actions work on dashcam clips unchanged.

---

## Milestone 12: Distribution & Polish

**Goal:** Spectator is installable by anyone. Platform binaries, documentation, agent skill file, and first public release.

**Depends on:** M0-M8 (core complete)

### Deliverables

- [ ] CI/CD: GitHub Actions release workflow
  - [ ] Cross-compilation matrix: linux-x86_64, macos-arm64, macos-x86_64, windows-x86_64
  - [ ] Build both `spectator-server` and `spectator-godot` per target
  - [ ] Package platform bundles (server binary + addon with matching GDExtension)
  - [ ] Upload to GitHub releases on tag push
- [ ] `cargo install spectator-server` works (publish to crates.io)
- [ ] Godot Asset Library submission (addon only, with instructions for server)
- [ ] Agent skill file (`skills/spectator.md`):
  - [ ] When to use each tool (decision tree)
  - [ ] Token-efficient patterns (summary → expand → inspect)
  - [ ] Common debugging workflows
  - [ ] Tool parameter cheat sheet
- [ ] README.md:
  - [ ] One-paragraph description
  - [ ] Quick start (3-step install)
  - [ ] MCP config snippets for Claude Code, Cursor
  - [ ] Architecture diagram
  - [ ] Link to docs
- [ ] End-to-end integration tests (Godot headless + server + MCP calls)
- [ ] 3D example project (`examples/3d-debug-demo/`) with enemies, patrols, walls, physics
- [ ] Performance profiling: verify <3ms per-frame overhead target
- [ ] Error message review: all errors have clear messages and actionable suggestions

### Exit Criteria

A developer can download a release, copy the addon into their Godot project, add the MCP config, and have their AI agent debugging spatial issues within 5 minutes. The skill file teaches the agent effective Spectator usage. Works on Linux, macOS, and Windows.

---

## Future (Post-1.0)

Not scheduled, but the architecture supports these without breaking changes:

### Performance Telemetry
- `spatial_perf` tool exposing FPS, physics tick time, draw calls, memory
- Data collected from Godot's `Performance` singleton and `Engine` class
- Opt-in, disabled by default

### Exported Build Support
- GDExtension autoload runs in release builds
- Build flag to include/exclude from exports
- Enables debugging platform-specific spatial issues (Steam Deck, mobile)

### Custom Data Extractors
- GDScript API: `SpectatorCollector.register_extractor(class_name, callable)`
- Callable receives a node, returns a Dictionary of custom data
- Custom data appears in `state` blocks alongside exported vars

### DAP Bridge
- Spatial watch conditions trigger DAP breakpoints
- "Break when enemy gets within 1m of wall" — spatial condition → code breakpoint
- Requires coordination with Agent Lens or Godot's built-in debugger

### Visual Replay
- In-editor playback of recordings with node positions rendered in the viewport
- Scrub bar, play/pause, speed control
- Agent markers shown as annotations in the viewport

### Multi-Scene Support
- Track scene transitions in recordings
- Scene-aware watches (survive scene changes if node paths match)
- Scene diff (what changed between scene loads)

### Streaming Updates
- Server-Sent Events or WebSocket transport for real-time spatial updates
- Agent receives continuous spatial stream instead of polling with delta
- Higher bandwidth but lower latency for time-sensitive debugging

---

## Dependency Graph

```
M0: Skeleton
 │
 ├── M1: Snapshot (first useful tool)
 │    │
 │    ├── M2: Inspect & Scene Tree
 │    │    │
 │    │    └── M10: Resource Inspection
 │    │
 │    ├── M3: Actions & Queries
 │    │    │
 │    │    └── M4: Deltas & Watches ←── M1
 │    │
 │    ├── M5: Configuration
 │    │
 │    ├── M6: Editor Dock ←── M4 (watch display)
 │    │    │
 │    │    └── M7: Recording Capture ←── M6
 │    │         │
 │    │         └── M8: Recording Analysis
 │    │              │
 │    │              └── M11: Dashcam Recording ←── M7, M8
 │    │
 │    └── M9: 2D Support ←── M1-M8
 │
 └── M12: Distribution & Polish ←── M0-M8
```

### Critical Path

**M0 → M1 → M3 → M4** is the critical path to a useful debugging loop (observe → act → check what changed). Everything else can be parallelized around this spine.

### Parallelizable Work

Once M1 is complete, these can proceed in parallel:
- **M2** (Inspect + Scene Tree) — independent of actions/deltas
- **M3** (Actions + Queries) — independent of inspect
- **M5** (Configuration) — independent of everything except snapshot

Once M3 is complete:
- **M4** (Deltas + Watches) and **M6** (Dock) can start

M7 and M8 (Recording) are sequential but can overlap with M9 (2D) and M10 (Resources).

Once M7 and M8 are complete:
- **M11** (Dashcam) extends recording with always-on capture
