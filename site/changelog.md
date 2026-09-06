---
description: "Theatre release notes — version history, new features, bug fixes, and breaking changes for each release."
---

# Changelog

All notable changes to Theatre are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Theatre uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Agent skills and client plugins

- Correct Director property and animation examples to use supported JSON values, and distinguish MCP diagnostics from CLI output.
- Update Stage guidance for clip configuration, visual artifacts, and observable error responses.
- Align contributor skills with current MCP handlers, recorder-owned settings, marker behavior, and Godot input ordering. Clarify the existing GDScript lifecycle and render-thread capture boundaries.
- Synchronize the corrected operating skills in the Claude and Codex plugin packages. Runtime behavior and the Godot 4.7 minimum are unchanged.

## [0.4.0] — 2026-09-05

### Compatibility

- Stage targets Godot 4.7. Update the installed tools and project addons together, then restart affected Godot and MCP processes.
- Automatic screenshot capture uses a supported asynchronous Compatibility/OpenGL path. Other renderers report images unavailable while spatial recording continues. Explicit synchronous recovery remains available but can stall gameplay.

### Recording and saved evidence

- Dashcam capture downsamples the existing viewport on the GPU and reads completed images asynchronously. It skips excess work before capture and reports unfinished images as gaps.
- The Spatial only preset disables new screenshots without discarding retained images. Presets preserve recording state and explicit project startup settings.
- Spatial collection reduces exported-property filtering overhead without caching live property state.
- Native controls distinguish Start/Stop, Mark, and Save now. They show the configured marker shortcut, acknowledge human actions, and provide saved clip references.
- Capture and feedback controls support corner or hidden placement. Hidden controls retain shortcut access.
- Shared dashcam settings provide validated partial updates, effective values, and actual buffered coverage. Lightweight and Detailed presets expose sampling trade-offs without starting recording.
- Saved clips retain run identity and scene-at-save context. Save failures remain distinct from recording state, and markers survive between spatial samples.
- Fresh CLI or MCP processes can inspect saved clips after the game closes, using the storage location published by a successful save.
- Optional movement evidence records selected InputMap strengths and CharacterBody3D contact facts. Saved snapshots and trajectories report sampling and truncation limits.

### Tool reliability

- Scene discovery returns absolute Godot paths that subsequent node inspection can reuse.
- Stage and Director output schemas use standard JSON Schema nullability. Structured results match their JSON text for strict MCP clients.
- Stale-addon identity errors direct users to deploy and restart the addon rather than change tool arguments.
- Director editor and daemon listeners bind explicitly to IPv4 loopback. Local callers still require trust.

### Installation and client integration

- Install, init, and deploy replace native payloads through staged files rather than truncating loaded binaries. Failed copies preserve existing destinations.
- Claude and Codex packages include self-contained Stage and Director operating skills and references, with shared marketplace entries.
- Optional feedback hooks honor explicit nested-project selection. Hook environment settings remain separate from the Stage server's environment, and installation does not grant hook trust.

### Known limits

- Recording still adds overhead. Bounded Linux/Godot 4.7.1 Voxlar samples improved Lightweight render-pacing p95 from 63.744 to 42.537 ms. These are short idle-scene measurements, not a universal performance guarantee.
- Windows and macOS qualification for these changes is not complete.
- The intermittent native AccessKit editor undo crash remains unresolved.


## [0.3.2] — 2026-03-16

### Infrastructure
- `theatre-cli`: 35 new CLI E2E tests covering `init`, `enable`, `rules`, `mcp`, `deploy`, and full lifecycle workflows
- Fix clippy warnings with `-D warnings` for CI
- Code formatting pass across workspace

## [0.2.2] — 2026-03-14

## [0.2.3] — 2026-03-15

## [0.3.0] — 2026-03-15

## [0.3.1] — 2026-03-15

## [0.3.2] — 2026-03-16

## [0.3.3] — 2026-08-15

## [0.3.4] — 2026-08-15
---

## [0.2.1] — 2026-03-14

### Site
- Architecture diagrams: replaced ASCII art with interactive Vue components

### Infrastructure
- Release workflow: cross-platform fix — replaced rsync with cp/find

---

## [0.2.0] — 2026-03-14

### Breaking Changes
- Renamed `spectator` → `stage` across the codebase: binary name, env vars, GDScript class names, addon paths

### Stage
- Agent-first CLI mode: `stage <tool>` runs a single MCP tool and exits with JSON on stdout
- Input injection actions on `spatial_action`: `action_press`, `action_release`, `inject_key`, `inject_mouse_button`
- GDScript code markers: `StageRuntime.marker()` for tagging moments from game code

### Director
- `batch`: `stop_on_error` parameter for partial-application mode

### Infrastructure
- One-liner install script (`install.sh`) for external users
- All crates updated to Rust edition 2024
- E2E test harness: `DirectorFixture` and `StageCliFixture` with windowed Godot support
- Full API docs alignment audit across all site pages

---

## [0.1.0] — 2026-03-12

Initial release of Theatre — an AI agent toolkit for Godot game engine.

### Stage

**9 MCP tools for observing running Godot games:**

- `spatial_snapshot` — Instant spatial snapshot of all tracked nodes. Supports `detail` levels (`summary`/`standard`/`full`), `token_budget` for response size control, `focal_node` for priority ordering, and `class_filter` for type filtering.

- `spatial_delta` — Changes-only response since a given frame. Token-efficient alternative to repeated snapshots. Supports `min_distance_change` threshold to filter physics noise.

- `spatial_query` — Geometric queries: `nearest`, `radius`, `area`, `raycast`, `path_distance`, `relationship`. Path distance uses the scene's NavigationRegion3D baked navmesh.

- `spatial_inspect` — Deep inspection of a single node. Returns all tracked properties, signal connections, children, and spatial context (nearby nodes, parent relationship).

- `spatial_watch` — Register nodes for continuous monitoring. Watched nodes appear in subsequent `spatial_delta` responses when their tracked properties change. Returns a stable `watch_id`.

- `spatial_config` — Configure tick rate, capture radius, tracked node types, and ring buffer depth. Changes take effect immediately.

- `spatial_action` — Set properties, call methods, or emit signals on running game nodes. For testing and hypothesis verification; changes are not persistent.

- `scene_tree` — Scene tree structure without spatial data. Compact and fast — good for orientation and path lookup.

- `clips` — Record gameplay to clip files, mark bug moments (F9), and query the spatial timeline. Supports condition filtering (`proximity`, `velocity_above`, `property_equals`) over frame ranges.

**GDExtension (stage):**
- Targets Godot 4.5+ with `compatibility_minimum = "4.5"`
- Uses `lazy-function-tables` for forward compatibility with 4.6+
- TCP listener on port 9077 (127.0.0.1 only)
- Ring buffer: 600 frames (~10s at 60Hz)
- Collection: O(n) over tracked nodes in `_physics_process`
- Graceful degradation: GDScript addon loads without crash if `.so` is missing

**Editor dock:**
- Connection status, tracked nodes, active watches count
- Keyboard shortcuts: F9 (save dashcam clip), F11 (pause game)
- Activity feed showing recent agent tool calls

**In-game overlay:**
- Dashcam status label (top-left)
- Marker flag button (top-left)
- Toast notifications for markers and clip saves (top-right)

### Director

**25+ operations across 8 domains:**

Scenes: `scene_create`, `scene_read`, `scene_list`, `scene_instance`, `scene_diff`

Nodes: `node_add`, `node_remove`, `node_set_properties`, `node_reparent`, `node_find`, `node_set_groups`, `node_set_script`, `node_set_meta`

Resources: `resource_read`, `material_create`, `shape_create`, `style_box_create`, `resource_duplicate`

TileMap/GridMap: `tilemap_set_cells`, `tilemap_get_cells`, `tilemap_clear`, `gridmap_set_cells`, `gridmap_get_cells`, `gridmap_clear`

Animation: `animation_create`, `animation_add_track`

Shaders: `visual_shader_create`

Physics: `physics_layer_names`, `physics_layer_set`, `physics_mask_set`

Wiring: `signal_connect`, `signal_disconnect`, `signal_list`

Batch: `batch`

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

[Unreleased]: https://github.com/nklisch/theatre/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/nklisch/theatre/releases/tag/v0.4.0
[0.3.4]: https://github.com/nklisch/theatre/releases/tag/v0.3.4
[0.3.3]: https://github.com/nklisch/theatre/releases/tag/v0.3.3
[0.3.2]: https://github.com/nklisch/theatre/releases/tag/v0.3.2
[0.3.1]: https://github.com/nklisch/theatre/releases/tag/v0.3.1
[0.3.0]: https://github.com/nklisch/theatre/releases/tag/v0.3.0
[0.2.3]: https://github.com/nklisch/theatre/releases/tag/v0.2.3
[0.2.2]: https://github.com/nklisch/theatre/releases/tag/v0.2.2
[0.2.1]: https://github.com/nklisch/theatre/releases/tag/v0.2.1
[0.2.0]: https://github.com/nklisch/theatre/releases/tag/v0.2.0
[0.1.0]: https://github.com/nklisch/theatre/releases/tag/v0.1.0
