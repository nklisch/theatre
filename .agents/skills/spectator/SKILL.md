---
name: spectator
description: Use Spectator's MCP tools to spatially debug a running Godot game. Invoke when the user is asking about game state, node positions, spatial bugs, physics, pathfinding, signals, or anything that requires understanding what's happening in the running game world. Also invoke when the user wants to set up recording sessions, watch for changes, or manipulate game state for debugging.
---

# Spectator — Spatial Debugging for Godot

Spectator gives you 9 MCP tools to observe and interact with a running Godot game. These tools see the game's spatial reality: positions, distances, relationships, physics, signals — organized in space, not as code.

**Prerequisite:** Spectator addon must be enabled in the Godot project and the game must be running (Play mode). If tools return `not_connected`, the game isn't running yet.

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
"Record while I reproduce the bug"        → recording
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
{
  "expand": "enemies",
  "detail": "standard"
}

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

Use after taking an action or advancing time to see what happened. Always compares against
the baseline from the most recent `spatial_snapshot` or `spatial_action`.

```jsonc
// See what changed since last snapshot (all defaults)
{}

// Filtered delta (reduces tokens)
{
  "groups": ["enemies"],
  "radius": 30.0
}
```

Parameters: `perspective` (camera/point), `radius` (default 50.0), `groups`, `class_filter`, `token_budget`.

Response includes: `from_frame`, `to_frame`, and any non-empty of: `moved`, `state_changed`, `entered`, `exited`, `signals_emitted`, `watch_triggers`.

**The act-then-delta pattern** — use `return_delta: true` on actions instead of a separate delta call:
```jsonc
// spatial_action with return_delta saves a round-trip
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
{
  "query_type": "nearest",
  "from": "player",
  "k": 5,
  "groups": ["enemies"]
}

// Can the enemy see the player?
{
  "query_type": "raycast",
  "from": "enemies/scout_02",
  "to": "player"
}

// Full relationship between two nodes
{
  "query_type": "relationship",
  "from": "enemies/scout_02",
  "to": "player"
}

// Navmesh path distance
{
  "query_type": "path_distance",
  "from": "enemies/guard_01",
  "to": "player"
}

// All enemies within 15 units of player
{
  "query_type": "radius",
  "from": "player",
  "radius": 15.0,
  "groups": ["enemies"]
}
```

`from` and `to` accept either a **node path** (`"player"`) or a **world position** (`[10.0, 0.0, 5.0]`).

## spatial_inspect — Deep Single Node

```jsonc
// Everything about a node
{ "node": "enemies/scout_02" }

// Specific categories only (cheaper)
{
  "node": "enemies/scout_02",
  "include": ["physics", "state"]
}

// Available categories:
// transform, physics, state, children, signals, script, spatial_context
```

**Useful include combos:**
- `["physics"]` — velocity, on_floor, collision_layer/mask → collision debugging
- `["state"]` — all exported vars → logic/AI state debugging
- `["children"]` — immediate children with key properties → hierarchy check
- `["signals"]` — connected signals + recent emissions → event flow debugging
- `["spatial_context"]` — nearby entities, areas, camera visibility → spatial context

## spatial_watch — Subscribe to Changes

```jsonc
// Watch a node for all changes
{
  "action": "add",
  "watch": { "node": "enemies/scout_02", "track": ["all"] }
}

// Conditional watch — only fires when health < 20
{
  "action": "add",
  "watch": {
    "node": "enemies/scout_02",
    "conditions": [{ "property": "health", "operator": "lt", "value": 20 }],
    "track": ["position", "state"]
  }
}

// Watch entire group
{
  "action": "add",
  "watch": { "node": "group:enemies", "track": ["position", "state"] }
}

// List active watches
{ "action": "list" }

// Remove all
{ "action": "clear" }
```

Watch triggers arrive in `spatial_delta` responses under `watch_triggers`. After setting up watches, call `spatial_delta` periodically to see if anything fired.

## spatial_action — Debugging Manipulation

```jsonc
// Pause the game
{ "action": "pause", "paused": true }

// Advance 30 frames while paused (0.5s at 60fps)
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
{
  "action": "set_property",
  "node": "enemies/scout_02",
  "property": "collision_mask",
  "value": 7
}

// Call a method
{
  "action": "call_method",
  "node": "enemies/scout_02",
  "method": "take_damage",
  "method_args": [50]
}

// Emit a signal
{
  "action": "emit_signal",
  "node": "enemies/scout_02",
  "signal": "health_changed",
  "args": [10]
}

// Spawn a scene
{
  "action": "spawn_node",
  "scene_path": "res://enemies/scout.tscn",
  "parent": "enemies",
  "name": "test_scout",
  "position": [10.0, 0.0, 0.0]
}
```

## scene_tree — Navigate Hierarchy

```jsonc
// Top-level structure
{ "action": "roots" }

// Recursive tree (depth 3 default)
{ "action": "subtree", "node": "enemies", "depth": 4 }

// Find all nodes with a script
{
  "action": "find",
  "find_by": "script",
  "find_value": "res://enemies/scout_ai.gd"
}

// Find all CharacterBody3D nodes
{
  "action": "find",
  "find_by": "class",
  "find_value": "CharacterBody3D"
}

// Parent chain for a node
{ "action": "ancestors", "node": "enemies/scout_02/NavAgent" }
```

## spatial_config — Session Setup

Call this at the start of a session to tune what Spectator tracks:

```jsonc
{
  "static_patterns": ["walls/*", "terrain/*", "props/*"],
  "state_properties": {
    "enemies": ["health", "alert_level", "current_target"],
    "CharacterBody3D": ["velocity"],
    "*": ["visible"]
  },
  "cluster_by": "group",
  "token_hard_cap": 3000
}
```

`state_properties` controls which exported vars appear in snapshot `state` blocks. Without this, you see all exported vars — often noisy. Configure per-group or per-class.

## recording — Human-Drives, Agent-Analyzes

The recording workflow: human reproduces the bug, agent analyzes the timeline.

```jsonc
// Check if recording is active (human may have started it with F8)
{ "action": "status" }

// Start recording (or the human hits F8)
{ "action": "start", "recording_name": "wall_clip_repro" }

// Start with custom capture config
{
  "action": "start",
  "recording_name": "detailed_run",
  "capture": {
    "capture_interval": 1,   // capture every N physics frames (default 1)
    "max_frames": 36000      // max frames to capture (default 36000)
  }
}

// List available recordings
{ "action": "list" }

// After human stops recording, analyze:

// See markers (F9 = human marker, system = auto-detected anomalies)
{ "action": "markers", "recording_id": "rec_001" }

// Spatial state at the marked frame
{ "action": "snapshot_at", "at_frame": 4582, "detail": "standard" }

// Find when enemy got within 0.5m of wall (across frame range)
{
  "action": "query_range",
  "recording_id": "rec_001",
  "from_frame": 4570,
  "to_frame": 4600,
  "node": "enemies/guard_01",
  "condition": { "type": "proximity", "target": "walls/*", "threshold": 0.5 }
}

// Compare before/after the bug
{ "action": "diff_frames", "frame_a": 4575, "frame_b": 4585 }

// Search for specific events
{
  "action": "find_event",
  "recording_id": "rec_001",
  "event_type": "signal",           // signal, property_change, collision, area_enter,
                                    // area_exit, node_added, node_removed, marker, input
  "event_filter": "health_changed", // substring match on event data (optional)
  "node": "enemies/guard_01",       // filter by node path (optional)
  "from_frame": 4500,               // optional frame range bounds
  "to_frame": 5000
}

// Mark your findings for the human to review
{
  "action": "add_marker",
  "marker_frame": 4578,
  "marker_label": "Root cause: collision_mask 3 missing layer 4 (walls)"
}
```

## Common Debugging Workflows

### Collision / Wall Clipping
```
1. spatial_config(static_patterns: ["walls/*"])
2. spatial_watch(node: "enemies/guard_01", track: ["position", "physics"])
3. [human reproduces bug, presses F9 at clip moment]
4. recording(action: "markers") → find the marked frame
5. recording(action: "query_range", condition: { type: "proximity", target: "walls/*", threshold: 0.5 })
6. spatial_inspect(node: "enemies/guard_01", include: ["physics"])
   → check collision_layer / collision_mask for mismatch
```

### Pathfinding Issues
```
1. spatial_query(query_type: "path_distance", from: "enemies/guard_01", to: "player")
   → check nav_distance vs straight_distance, traversable
2. spatial_inspect(node: "enemies/guard_01", include: ["children", "spatial_context"])
   → NavigationAgent3D.distance_remaining, nearest_navmesh_edge_dist
3. spatial_query(query_type: "relationship", from: "enemies/guard_01", to: "walls/segment_04")
   → distance, bearing, line_of_sight
```

### AI State Machine Debugging
```
1. spatial_config(state_properties: { enemies: ["state", "alert_level", "current_target"] })
2. spatial_snapshot(groups: ["enemies"], detail: "standard")
   → see all enemies' state at once
3. spatial_watch(node: "enemies/guard_01",
     conditions: [{ property: "alert_level", operator: "changed" }])
4. spatial_delta() → catch state transitions as they happen
5. spatial_inspect(node: "enemies/guard_01", include: ["state", "signals"])
   → exact exported vars + recent signal emissions
```

### Physics Debugging
```
1. spatial_action(action: "pause", paused: true)
2. spatial_inspect(node: "enemies/scout_02", include: ["physics"])
   → velocity, on_floor, collision_layer, collision_mask, floor_normal
3. spatial_action(action: "advance_frames", frames: 1)
4. spatial_delta() → see exactly what changed in that one frame
5. Repeat steps 3-4 to step frame-by-frame
```

## Reading the Spatial Output

**Bearings** are relative to the perspective entity's facing direction:
```
ahead, ahead_left, ahead_right, left, right, behind, behind_left, behind_right
```

**Elevation** (3D only): `level` (within ±2m), `above_5m`, `below_2m`

**`relative` block** on each entity:
```jsonc
{
  "distance": 7.2,          // straight-line distance in world units
  "bearing": "ahead_left",  // relative to perspective facing
  "bearing_deg": 322,       // exact degrees (0 = ahead, clockwise)
  "elevation": "level",
  "occluded": false          // is camera view blocked?
}
```

**`global_position`** is the world position (`[x, y, z]` in 3D, `[x, y]` in 2D).

## Error Reference

| Error | Meaning | Fix |
|---|---|---|
| `not_connected` | Game not running or addon not enabled | Start the game in Godot |
| `scene_not_loaded` | Between scene transitions | Wait for scene to finish loading |
| `node_not_found` | Path doesn't exist | Use `scene_tree(action: "find")` to locate the node |
| `timeout` | Game frozen or at a breakpoint | Check if game is paused/broken |
| `recording_active` | Already recording | Stop current recording first |
| `budget_exceeded` | Too many nodes, too large | Reduce radius, add group filter, or use summary detail |
