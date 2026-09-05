---
name: theatre-stage
description: >
  Spatial debugging for running Godot games via Stage MCP tools or CLI.
  ACTIVATE when: user mentions game state, node positions, spatial bugs,
  physics issues, pathfinding problems, collision debugging, AI behavior,
  signal tracing, "take a snapshot", "what's happening in the game",
  clip/recording analysis, watch subscriptions, or any task that requires
  observing or interacting with a live Godot game world. Also activate
  for frame-by-frame debugging, teleporting nodes, pausing/advancing time,
  or injecting input. Do NOT activate for editing .tscn files or creating
  scenes — use theatre-director for that.
---

# Stage — Spatial Debugging for Godot

Stage is part of the **Theatre** toolkit (alongside Director). It observes and interacts with a running Godot game through spatial state, current viewport images, bounded runtime diagnostics, retained clips, and explicit debug actions.

**Two interfaces, different session lifetimes:**

| Interface | When to use | Example |
|---|---|---|
| MCP tools | Agent has MCP connection to stage | `spatial_snapshot(detail: "summary")` |
| CLI | Agent uses bash, no MCP server running | `stage spatial_snapshot '{"detail":"summary"}'` |

**CLI basics:**
```bash
stage <tool> '<json-params>'           # direct invocation
echo '{"detail":"summary"}' | stage spatial_snapshot  # stdin pipe
stage --help                           # list all tools
stage --version                        # {"version": "0.1.0"}
```

CLI tool results are JSON to stdout. Errors are JSON to stdout with exit code 1 (runtime) or 2 (usage). Logs go to stderr.

Each CLI call starts a fresh session. `spatial_delta`, all `spatial_watch`
operations, `spatial_config` updates, and actions with `return_delta: true`
return `persistent_session_required` (exit 2) before connecting or acting.
A snapshot in one CLI invocation cannot establish another invocation's baseline.
`spatial_config '{}'` still reads project defaults; put reusable defaults in
`stage.toml`. Ordinary snapshots, inspection, queries, actions without delta,
and addon-owned clip operations remain available.

For stateful workflows, configure your MCP client with command `stage` and
arguments `["serve"]`. Keep snapshot, watch/config, action, and delta calls in
that same MCP session. `stage serve` speaks MCP over stdio; it is not a shell
session command for passing subsequent CLI calls into. In shell-only workflows,
act without `return_delta`, then inspect or snapshot the result explicitly.

**Prerequisite for live tools:** The Stage addon must be enabled and the game must be running. If live tools return `not_connected` or `connection_failed`, use `runtime_status` and start the selected saved scene through Director `editor_run` or Godot. Project-local `feedback` remains available without a running game.

## When to Use Which Tool

```
"Which project and run is connected?"     → runtime_status
"What errors occurred in this run?"        → runtime_diagnostics
"Show the latest completed render"         → viewport
"What's in the scene right now?"          → spatial_snapshot
"What changed since last time?"           → spatial_delta
"What's near X? Can A see B?"             → spatial_query
"Tell me everything about this node"      → spatial_inspect
"Alert me when health drops below 20"     → spatial_watch
"Teleport/pause/run bounded input"         → spatial_action
"How is this scene structured?"           → scene_tree
"Configure what to track"                 → spatial_config
"Mark this moment / save a clip"          → clips
"What did the developer share?"           → feedback
```

## Standard Opening Move

Always start cheap and drill down:

```
1. runtime_status()                              → verify project, run, scene, readiness
2. spatial_snapshot(detail: "summary")         → cheap scene overview
3. spatial_snapshot(expand: "enemies")          → focused entity details
4. spatial_inspect(node: "enemies/scout_02")    → deep dive
```

If a result includes `feedback_notice`, check `feedback(action: "status")` and
retrieve the matching item before continuing past the human's observation.
Retrieval is non-destructive; handle the item explicitly after addressing it.

Never start with `detail: "full"` on the full scene — that's expensive and usually unnecessary.

## spatial_snapshot — Scene Overview

```jsonc
// Minimum: what's in the scene?
{ "detail": "summary" }

// Standard view with filters
{
  "detail": "standard",
  "groups": ["enemies"],
  "radius": 30.0,
  "perspective": "camera"
}

// Drill into a summary cluster
{ "expand": "enemies", "detail": "standard" }

// From a specific node's perspective
{
  "perspective": "node",
  "focal_node": "player",
  "detail": "standard",
  "radius": 20.0
}
```

**`detail` tiers:**
- `summary` (~200t): clusters with counts, nearest/farthest, brief state summary. Use first.
- `standard` (~400-800t): per-entity positions, bearings, state, recent signals. Use for most debugging.
- `full` (~1000t+): adds full transforms, physics, children, scripts, static listings. Use only when needed.

**Filtering reduces tokens and noise:**
- `groups: ["enemies"]` — only nodes in the "enemies" group
- `class_filter: ["CharacterBody3D"]` — only that class
- `radius: 20.0` — only within 20 units

## spatial_delta — What Changed?

Use in persistent MCP after taking an action or advancing time. Compares against the baseline established by `spatial_snapshot` in that same session, then updated by deltas (including action-returned deltas). One-shot CLI delta calls are rejected.

```jsonc
// See what changed (all defaults)
{}

// Filtered delta
{ "groups": ["enemies"], "radius": 30.0 }
```

Parameters: `perspective` (camera/point), `radius` (default 50.0), `groups`, `class_filter`, `token_budget`.

Response includes: `from_frame`, `to_frame`, and any non-empty of: `moved`, `state_changed`, `entered`, `exited`, `signals_emitted`, `watch_triggers`.

**The act-then-delta pattern** — use `return_delta: true` on actions instead of a separate delta call:
```jsonc
{
  "action": "teleport",
  "node": "enemies/scout_02",
  "position": [5.0, 0.0, -3.0],
  "return_delta": true
}
```

## spatial_query — Targeted Spatial Questions

```jsonc
// What's near the player?
{ "query_type": "nearest", "from": "player", "k": 5, "groups": ["enemies"] }

// Can the enemy see the player?
{ "query_type": "raycast", "from": "enemies/scout_02", "to": "player" }

// Full relationship between two nodes
{ "query_type": "relationship", "from": "enemies/scout_02", "to": "player" }

// Navmesh path distance
{ "query_type": "path_distance", "from": "enemies/guard_01", "to": "player" }

// All enemies within 15 units of player
{ "query_type": "radius", "from": "player", "radius": 15.0, "groups": ["enemies"] }
```

`from` and `to` accept either a **node path** (`"player"`) or a **world position** (`[10.0, 0.0, 5.0]`).

## spatial_inspect — Deep Single Node

```jsonc
// Everything about a node
{ "node": "enemies/scout_02" }

// Specific categories only (cheaper)
{ "node": "enemies/scout_02", "include": ["physics", "state"] }

// Available categories:
// transform, physics, state, children, signals, script, spatial_context, resources
```

**Useful include combos:**
- `["physics"]` — velocity, on_floor, collision_layer/mask
- `["state"]` — all exported vars
- `["children"]` — immediate children with key properties
- `["signals"]` — connected signals + recent emissions
- `["spatial_context"]` — nearby entities, areas, camera visibility

## spatial_watch — Subscribe to Changes

```jsonc
// Watch a node for all changes
{ "action": "add", "watch": { "node": "enemies/scout_02", "track": ["all"] } }

// Conditional watch — fires when health < 20
{
  "action": "add",
  "watch": {
    "node": "enemies/scout_02",
    "conditions": [{ "property": "health", "operator": "lt", "value": 20 }],
    "track": ["position", "state"]
  }
}

// Watch entire group
{ "action": "add", "watch": { "node": "group:enemies", "track": ["position", "state"] } }

// List active watches
{ "action": "list" }

// Remove all
{ "action": "clear" }
```

Watch triggers arrive in `spatial_delta` responses under `watch_triggers`.

**Note:** Watches require a persistent MCP session. One-shot CLI watch operations are rejected; they cannot access another session's subscriptions.

## spatial_action — Debugging Manipulation

```jsonc
// Pause the game
{ "action": "pause", "paused": true }

// Advance 30 frames while paused
{ "action": "advance_frames", "frames": 30 }

// Teleport a node
{
  "action": "teleport",
  "node": "enemies/scout_02",
  "position": [5.0, 0.0, -3.0],
  "rotation_deg": 180,
  "return_delta": true
}

// Change a property
{ "action": "set_property", "node": "enemies/scout_02", "property": "collision_mask", "value": 7 }

// Call a method
{ "action": "call_method", "node": "enemies/scout_02", "method": "take_damage", "args": [50] }

// Emit a signal
{ "action": "emit_signal", "node": "enemies/scout_02", "signal": "health_changed", "args": [10] }

// Spawn a scene
{
  "action": "spawn_node",
  "scene_path": "res://enemies/scout.tscn",
  "parent": "enemies",
  "name": "test_scout",
  "position": [10.0, 0.0, 0.0]
}

// Advance half a second while paused
{ "action": "advance_time", "seconds": 0.5 }

// Remove a node
{ "action": "remove_node", "node": "enemies/scout_02" }

// Simulate input action
{ "action": "action_press", "input_action": "jump" }
{ "action": "action_release", "input_action": "jump" }

// Inject key event
{ "action": "inject_key", "keycode": "space", "pressed": true }

// Inject mouse button event
{ "action": "inject_mouse_button", "button": "left", "pressed": true, "position": [400, 300] }

// Run a bounded InputMap sequence while already paused
{
  "action": "interaction_sequence",
  "steps": [
    { "press": [{ "action_name": "move_right" }], "frames": 20 },
    { "press": [{ "action_name": "jump" }], "frames": 1 },
    { "release": ["jump", "move_right"], "frames": 10 }
  ]
}
```

An interaction sequence accepts a bounded step and frame count, keeps the game
paused, and releases sequence-held actions on supported completion and cleanup
paths. It does not make gameplay deterministic. If the engine is stopped or hung
inside a native debugger, its cleanup callback cannot run.

## Current Runtime Evidence

Use `runtime_status` before a run-sensitive workflow. It reports the actual
project, process, `run_id`, current scene, and readiness. A Director launch
request and a TCP connection do not by themselves establish readiness.

Use `runtime_diagnostics` for bounded errors, warnings, script errors, and shader
errors captured after the Stage autoload registered its Logger. Reads do not
consume diagnostics. The queue survives client reconnects but not a game restart.
It does not recover early engine initialization output, suppressed log streams,
or unavailable release backtraces.

Use `viewport` for a bounded JPEG of the latest completed root-viewport render.
It does not start recording, save a clip, or use the recorder. Readback counters
show provenance but do not make pixels atomic with a separate spatial query.
Headless or empty-pixel responses leave spatial observation available.

## feedback — Human Context

```jsonc
{ "action": "status" }
{ "action": "retrieve", "feedback_id": "feedback_..." }
{ "action": "handle", "feedback_id": "feedback_..." }
```

Feedback is retained under the selected project's `.theatre/feedback` directory
and remains readable after the game exits. It can contain runtime or editor
selection/pointer context, an optional JPEG, and a note. Retrieval does not handle
or delete evidence. Handling suppresses pending notices for every reader but
keeps retrieval available; deletion is a separate explicit action.

## scene_tree — Navigate Hierarchy

```jsonc
// Top-level structure
{ "action": "roots" }

// Immediate children
{ "action": "children", "node": "enemies" }

// Recursive tree (depth 3 default)
{ "action": "subtree", "node": "enemies", "depth": 4 }

// Find nodes by class
{ "action": "find", "find_by": "class", "find_value": "CharacterBody3D" }

// Find nodes by script
{ "action": "find", "find_by": "script", "find_value": "res://enemies/scout_ai.gd" }

// Parent chain
{ "action": "ancestors", "node": "enemies/scout_02/NavAgent" }
```

## spatial_config — Session Setup

Call at the start of a persistent MCP session to tune what Stage tracks.
One-shot CLI accepts an empty configuration read, not updates; use `stage.toml`
for defaults shared by future invocations:

```jsonc
{
  "static_patterns": ["walls/*", "terrain/*", "props/*"],
  "state_properties": {
    "enemies": ["health", "alert_level", "current_target"],
    "CharacterBody3D": ["velocity"],
    "*": ["visible"]
  },
  "cluster_by": "group",
  "bearing_format": "cardinal",
  "token_hard_cap": 3000,
  "poll_interval": 1,
  "expose_internals": false
}
```

`state_properties` controls which exported vars appear in snapshot `state` blocks.

## clips — Mark, Save, Analyze

Clips are captured by the dashcam ring buffer. Mark a moment to save; analyze saved clips.

```jsonc
// Check dashcam buffer state
{ "action": "status" }

// Mark a moment — triggers automatic clip save
{ "action": "add_marker", "marker_label": "wall_clip_repro" }

// Force-save the current buffer
{ "action": "save", "marker_label": "manual save" }

// List saved clips
{ "action": "list" }

// See markers in a clip
{ "action": "markers", "clip_id": "clip_001a2b3c" }
// Note: marker entries have a "source" field: "human" (F9), "agent" (MCP add_marker),
// "system" (automatic dashcam trigger), or "code" (StageRuntime.marker() in game script).
// Code markers may be "system" tier (rate-limited), "deliberate" (always triggers),
// or "silent" (annotation only — attached to clips triggered by other means).

// Spatial state at a frame (omit clip_id for most recent)
{ "action": "snapshot_at", "at_frame": 4582, "detail": "standard" }

// Find when enemy got within 0.5m of wall
{
  "action": "query_range",
  "from_frame": 4570, "to_frame": 4600,
  "node": "enemies/guard_01",
  "condition": { "type": "proximity", "target": "walls/*", "threshold": 0.5 }
}

// Compare before/after
{ "action": "diff_frames", "frame_a": 4575, "frame_b": 4585 }

// Search for events
{
  "action": "find_event",
  "event_type": "signal",
  "event_filter": "health_changed",
  "node": "enemies/guard_01",
  "from_frame": 4500, "to_frame": 5000
}

// Delete a clip
{ "action": "delete", "clip_id": "clip_001a2b3c" }

// Node trajectory over time
{ "action": "trajectory", "node": "enemies/guard_01", "from_frame": 4500, "to_frame": 5000 }

// Screenshot at a frame
{ "action": "screenshot_at", "at_frame": 4582 }

// List available screenshots
{ "action": "screenshots", "clip_id": "clip_001a2b3c" }
```

## Common Debugging Workflows

### Collision / Wall Clipping
```
1. spatial_config(static_patterns: ["walls/*"])
2. spatial_watch(node: "enemies/guard_01", track: ["position", "physics"])
3. [human reproduces bug, presses F9]
4. clips(action: "markers") → find the marked frame
5. clips(action: "query_range", condition: { type: "proximity", target: "walls/*", threshold: 0.5 })
6. spatial_inspect(node: "enemies/guard_01", include: ["physics"])
```

### Pathfinding Issues
```
1. spatial_query(query_type: "path_distance", from: "guard_01", to: "player")
2. spatial_inspect(node: "guard_01", include: ["children", "spatial_context"])
3. spatial_query(query_type: "relationship", from: "guard_01", to: "walls/segment_04")
```

### AI State Machine Debugging
```
1. spatial_config(state_properties: { enemies: ["state", "alert_level", "current_target"] })
2. spatial_snapshot(groups: ["enemies"], detail: "standard")
3. spatial_watch(node: "guard_01", conditions: [{ property: "alert_level", operator: "changed" }])
4. spatial_delta() → catch state transitions
5. spatial_inspect(node: "guard_01", include: ["state", "signals"])
```

### Physics Debugging (frame-by-frame)
```
1. spatial_action(action: "pause", paused: true)
2. spatial_inspect(node: "scout_02", include: ["physics"])
3. spatial_action(action: "advance_frames", frames: 1)
4. spatial_delta() → see exactly what changed
5. Repeat 3-4
```

## Reading Spatial Output

**Bearings** — relative to perspective entity's facing:
`ahead`, `ahead_left`, `ahead_right`, `left`, `right`, `behind`, `behind_left`, `behind_right`

**Elevation** (3D only): `level` (±2m), `above_5m`, `below_2m`

**`relative` block** on each entity:
```jsonc
{ "distance": 7.2, "bearing": "ahead_left", "bearing_deg": 322, "elevation": "level", "occluded": false }
```

**`global_position`** — world position (`[x, y, z]` 3D, `[x, y]` 2D).

## Error Reference

| Error | Meaning | Fix |
|---|---|---|
| `not_connected` / `connection_failed` | Game not running or addon not enabled | Check `runtime_status`; start the selected scene through Director or Godot |
| `unknown_tool` | Invalid tool name (CLI only) | Check `stage --help` |
| `invalid_json` | Bad JSON params (CLI only) | Fix JSON syntax |
| `persistent_session_required` | One-shot call needs retained server state (CLI only) | Use the same `stage serve` MCP session; for one-shot actions omit `return_delta` |
| `scene_not_loaded` | Between scene transitions | Wait for scene to load |
| `node_not_found` | Path doesn't exist | Use `scene_tree(action: "find")` |
| `timeout` | Game frozen or at breakpoint | Check if game is paused |
| `dashcam_disabled` | Dashcam not active | Check spatial_config |
| `budget_exceeded` | Too many nodes | Reduce radius, add filters, use summary |
