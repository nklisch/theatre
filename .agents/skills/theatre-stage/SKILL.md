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

Stage is part of the **Theatre** toolkit (alongside Director). It gives you 9 MCP tools to observe and interact with a running Godot game: positions, distances, relationships, physics, signals — organized in space, not as code.

**Two interfaces, identical capabilities:**

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

All CLI output is JSON to stdout. Errors are JSON to stdout with exit code 1 (runtime) or 2 (usage). Logs go to stderr.

**Prerequisite:** Stage addon must be enabled in the Godot project and the game must be running. If tools return `not_connected` / `connection_failed`, the game isn't running.

## When to Use Which Tool

```
"What's in the scene right now?"          → spatial_snapshot
"What changed since last time?"           → spatial_delta
"What's near X? Can A see B?"             → spatial_query
"Tell me everything about this node"      → spatial_inspect
"Alert me when health drops below 20"     → spatial_watch
"Teleport/pause/set property"             → spatial_action
"How is this scene structured?"           → scene_tree
"Configure what to track"                 → spatial_config
"Mark this moment / save a clip"          → clips
```

## Standard Opening Move

Always start cheap and drill down:

```
1. spatial_snapshot(detail: "summary")         → ~200 tokens, scene overview
2. spatial_snapshot(expand: "enemies")          → ~400 tokens, enemy details
3. spatial_inspect(node: "enemies/scout_02")    → ~800 tokens, deep dive
```

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

Use after taking an action or advancing time. Compares against the baseline from the most recent `spatial_snapshot` or `spatial_action`.

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

**Note:** Watches require a persistent session (MCP mode). In CLI one-shot mode, watches only exist for the duration of a single call.

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

// Advance time (physics frames at given delta)
{ "action": "advance_time", "duration": 0.5, "delta": 0.016 }

// Remove a node
{ "action": "remove_node", "node": "enemies/scout_02" }

// Simulate input action
{ "action": "action_press", "input_action": "jump" }
{ "action": "action_release", "input_action": "jump" }

// Inject key event
{ "action": "inject_key", "keycode": "space", "pressed": true }

// Inject mouse button event
{ "action": "inject_mouse_button", "button": "left", "pressed": true, "position": [400, 300] }
```

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

Call at the start of a session to tune what Stage tracks:

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
| `not_connected` / `connection_failed` | Game not running or addon not enabled | Start the game in Godot |
| `unknown_tool` | Invalid tool name (CLI only) | Check `stage --help` |
| `invalid_json` | Bad JSON params (CLI only) | Fix JSON syntax |
| `scene_not_loaded` | Between scene transitions | Wait for scene to load |
| `node_not_found` | Path doesn't exist | Use `scene_tree(action: "find")` |
| `timeout` | Game frozen or at breakpoint | Check if game is paused |
| `dashcam_disabled` | Dashcam not active | Check spatial_config |
| `budget_exceeded` | Too many nodes | Reduce radius, add filters, use summary |
