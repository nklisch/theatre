# Godot Spatial Viewport — MCP Tool Contracts

## Project: GodotView (working name)

A Rust MCP server providing token-efficient spatial debugging for AI coding agents working in Godot. Connects to a lightweight GDScript addon inside the engine via TCP, exposes spatial state as MCP tools.

**Design principle:** The spatial view is the primary interface for debugging _game state in space_. It carries transforms, properties, signals, custom data — everything about a node's runtime reality — organized spatially rather than as code inspection. DAP/Agent Lens handles code debugging (breakpoints, stack frames, variable inspection). These are complementary, not overlapping.

---

## MCP Tools

### 1. `spatial_snapshot`

The primary view. Returns a token-budgeted representation of the scene from a spatial perspective.

**When an agent should use this:** "What does the scene look like right now?" — the spatial equivalent of opening a file.

#### Parameters

```jsonc
{
  // Where to look from. Defaults to active camera.
  "perspective": {
    "type": "string",
    "enum": ["camera", "node", "point"],
    "default": "camera"
  },

  // Required if perspective is "node" — path like "enemies/scout_02"
  "focal_node": {
    "type": "string",
    "optional": true
  },

  // Required if perspective is "point" — raw world position
  "focal_point": {
    "type": "array",  // [x, y, z]
    "optional": true
  },

  // Max distance from focal point/camera to include. Default 50.0
  "radius": {
    "type": "number",
    "default": 50.0
  },

  // Detail tier for the response
  "detail": {
    "type": "string",
    "enum": ["summary", "standard", "full"],
    "default": "standard"
  },

  // Filter by group membership
  "groups": {
    "type": "array",
    "items": "string",
    "optional": true
  },

  // Filter by node class (e.g. "CharacterBody3D", "Area3D")
  "class_filter": {
    "type": "array",
    "items": "string",
    "optional": true
  },

  // Include nodes outside the camera frustum (within radius).
  // Default false — only visible nodes.
  "include_offscreen": {
    "type": "boolean",
    "default": false
  },

  // --- Token Budget & Pagination ---

  // Soft token budget for the response. Controls how many entities are
  // included before truncation. Server enforces a hard cap (default 5000,
  // configurable via spatial_config). If omitted, uses detail-tier defaults.
  // detail controls WHAT fields per entity; token_budget controls HOW MANY
  // entities fit in the response.
  "token_budget": {
    "type": "integer",
    "optional": true,
    "description": "Approximate max tokens for the response payload"
  },

  // Continuation cursor from a previous truncated response.
  // When present, returns the next page of entities from the same
  // snapshot frame. All other parameters are inherited from the
  // original request — only cursor should be provided.
  "cursor": {
    "type": "string",
    "optional": true
  },

  // Expand a specific cluster from a previous summary response.
  // Returns standard/full detail for just that cluster's entities
  // without re-fetching the full scene. The label must match a
  // cluster label from the prior snapshot.
  "expand": {
    "type": "string",
    "optional": true,
    "description": "Cluster label to drill into from a previous summary"
  }
}
```

#### Response Shape — `detail: "summary"` (~150-300 tokens)

```jsonc
{
  "frame": 2847,
  "timestamp_ms": 47450,
  "perspective": {
    "position": [0.0, 1.8, 0.0],
    "facing": "north",         // cardinal approximation
    "facing_deg": 5.2          // exact yaw for precision work
  },

  // Clustered by group or spatial proximity
  "clusters": [
    {
      "label": "enemies",
      "count": 3,
      "nearest": { "node": "enemy/scout_02", "dist": 7.2, "bearing": "ahead_left" },
      "farthest_dist": 22.1,
      "summary": "2 idle, 1 patrol"   // derived from state props
    },
    {
      "label": "pickups",
      "count": 2,
      "nearest": { "node": "pickups/health_01", "dist": 3.1, "bearing": "right" },
      "summary": "1 health, 1 ammo"
    },
    {
      "label": "static_geometry",
      "count": 24,
      "note": "unchanged"            // suppressed — no detail unless asked
    }
  ],

  // Notable events since server started tracking (or since last snapshot)
  "recent_events": [
    { "frame": 2840, "event": "enemy/guard_01 entered area 'patrol_zone_b'" },
    { "frame": 2845, "event": "pickups/ammo_03 removed (collected)" }
  ],

  "total_nodes_tracked": 29,
  "total_nodes_visible": 14,

  // --- Pagination ---
  // Present when entity list was truncated to fit token budget
  "pagination": {                        // omitted if not truncated
    "truncated": true,
    "showing": 14,
    "total": 29,
    "cursor": "snap_2847_p2",            // pass to next call to get more
    "omitted_nearest_dist": 15.2         // closest entity that didn't make the cut
  },

  // Token accounting
  "budget": {
    "used": 280,                         // approximate tokens in this response
    "limit": 500,                        // effective budget for this call
    "hard_cap": 5000                     // server max (configurable via spatial_config)
  }
}
```

#### Response Shape — `detail: "standard"` (~400-800 tokens)

Everything in summary, plus individual entries for dynamic nodes:

```jsonc
{
  // ...summary fields...

  "entities": [
    {
      "path": "enemies/scout_02",
      "class": "CharacterBody3D",
      "rel": {                          // relative to perspective
        "dist": 7.2,
        "bearing": "ahead_left",
        "bearing_deg": 322,
        "elevation": "level",           // or "above"/"below" + degrees
        "occluded": false
      },
      "abs": [12.4, 0.0, -8.2],        // world position, compact
      "rot_y": 135,                     // yaw only unless 6DOF relevant
      "velocity": [1.2, 0.0, -0.8],    // only if moving
      "groups": ["enemies", "patrol_route_b"],
      "state": {                        // exported vars / custom properties
        "health": 80,
        "alert_level": "suspicious",
        "current_target": null
      },
      "signals_recent": [               // signals emitted since last query
        { "signal": "health_changed", "frame": 2830 }
      ]
    },
    // ...more entities
  ],

  // Static nodes listed minimally (path + position only, no state)
  "static_summary": {
    "count": 24,
    "categories": {
      "wall_segments": 12,
      "props": 8,
      "lights": 4
    }
  },

  // Pagination & budget — same shape as summary tier
  "pagination": { /* ... */ },
  "budget": { /* ... */ }
}
```

#### Response Shape — `detail: "full"` (~1000+ tokens)

Adds to standard:

```jsonc
{
  // ...standard fields...

  "entities": [
    {
      // ...standard entity fields...

      "transform": {                    // full transform data
        "origin": [12.4, 0.0, -8.2],
        "basis": [[1,0,0],[0,1,0],[0,0,1]],  // only if non-identity rotation matters
        "scale": [1.0, 1.0, 1.0]
      },
      "physics": {
        "velocity": [1.2, 0.0, -0.8],
        "on_floor": true,
        "collision_layer": 1,
        "collision_mask": 3
      },
      "children": [                     // immediate children with types
        { "name": "CollisionShape3D", "class": "CollisionShape3D" },
        { "name": "MeshInstance3D", "class": "MeshInstance3D" },
        { "name": "NavigationAgent3D", "class": "NavigationAgent3D" }
      ],
      "script": "res://enemies/scout_ai.gd",
      "signals_connected": ["health_changed", "target_acquired", "path_completed"],
      "all_exported_vars": {
        "health": 80,
        "max_health": 100,
        "alert_level": "suspicious",
        "patrol_speed": 3.5,
        "chase_speed": 7.0,
        "current_target": null,
        "detection_radius": 15.0
      }
    }
  ],

  // Static nodes get full listing at this tier
  "static_nodes": [
    { "path": "walls/segment_01", "class": "StaticBody3D", "pos": [0, 0, -5], "aabb": [4, 3, 0.5] },
    // ...
  ]
}
```

---

### 2. `spatial_delta`

Returns only what changed since the last query. Designed for agent loops where the agent takes an action, then checks what happened.

**When an agent should use this:** "I just told the enemy to move — did it? What changed?"

#### Parameters

```jsonc
{
  // Frame number from a previous snapshot/delta to diff against.
  // If omitted, diffs against the last query this session made.
  "since_frame": {
    "type": "integer",
    "optional": true
  },

  // Same perspective/radius/filter options as spatial_snapshot
  "perspective": { "type": "string", "default": "camera" },
  "radius": { "type": "number", "default": 50.0 },
  "groups": { "type": "array", "optional": true },

  // Token budget — same semantics as spatial_snapshot.
  // Deltas are usually small, but a long time gap or many watched
  // nodes can produce large responses.
  "token_budget": { "type": "integer", "optional": true }
}
```

#### Response Shape (~100-400 tokens for typical frames)

```jsonc
{
  "from_frame": 2847,
  "to_frame": 2863,
  "dt_ms": 267,

  "moved": [
    {
      "path": "enemies/scout_02",
      "pos": [13.1, 0.0, -9.0],          // new position
      "delta_pos": [0.7, 0.0, -0.8],     // movement vector
      "dist_to_focal": 8.4               // updated distance
    }
  ],

  "state_changed": [
    {
      "path": "enemies/scout_02",
      "changes": { "alert_level": ["suspicious", "alert"] }  // [old, new]
    }
  ],

  "entered": [
    { "path": "enemies/reinforcement_01", "class": "CharacterBody3D", "pos": [30, 0, -15] }
  ],

  "exited": [
    { "path": "pickups/ammo_03", "reason": "queue_freed" }
  ],

  "signals_emitted": [
    { "path": "enemies/scout_02", "signal": "target_acquired", "args": ["player"], "frame": 2855 }
  ],

  "static_changed": false   // true if any static geometry moved (rare, notable)
}
```

---

### 3. `spatial_query`

Targeted spatial questions. Instead of getting the whole viewport and filtering, the agent asks a specific question.

**When an agent should use this:** "What's near the player?" / "Is there line of sight between A and B?" / "What's in this area?"

#### Parameters

```jsonc
{
  "query_type": {
    "type": "string",
    "enum": [
      "nearest",         // K nearest nodes to a point/node
      "radius",          // all nodes within radius of point/node
      "raycast",         // line-of-sight / collision check between two points
      "area",            // nodes within an AABB or sphere
      "path_distance",   // navigation mesh distance between two nodes
      "relationship"     // spatial relationship between two specific nodes
    ]
  },

  // Origin — either a node path or a world position
  "from": {
    "type": "string | array",  // "player" or [10, 0, 5]
    "required": true
  },

  // Target — for raycast and relationship queries
  "to": {
    "type": "string | array",
    "optional": true
  },

  // For nearest/radius queries
  "k": { "type": "integer", "default": 5 },
  "radius": { "type": "number", "default": 20.0 },

  // Filters (same as snapshot)
  "groups": { "type": "array", "optional": true },
  "class_filter": { "type": "array", "optional": true }
}
```

#### Response Examples

**`nearest` query:**
```jsonc
{
  "query": "nearest",
  "from": "player",
  "results": [
    { "path": "pickups/health_01", "dist": 3.1, "bearing": "right", "class": "Area3D" },
    { "path": "enemies/scout_02", "dist": 7.2, "bearing": "ahead_left", "class": "CharacterBody3D" },
    { "path": "props/barrel_07", "dist": 8.9, "bearing": "behind_right", "class": "StaticBody3D" }
  ]
}
```

**`raycast` query:**
```jsonc
{
  "query": "raycast",
  "from": "enemies/scout_02",
  "to": "player",
  "result": {
    "clear": false,
    "blocked_by": "walls/segment_04",
    "blocked_at": [8.2, 1.0, -4.1],
    "total_distance": 15.3,
    "clear_distance": 6.7
  }
}
```

**`relationship` query:**
```jsonc
{
  "query": "relationship",
  "from": "enemies/scout_02",
  "to": "player",
  "result": {
    "distance": 15.3,
    "bearing_from_a": "behind_right",      // player is behind-right of scout
    "bearing_from_b": "ahead_left",        // scout is ahead-left of player
    "elevation_diff": 0.0,
    "line_of_sight": false,
    "occluder": "walls/segment_04",
    "nav_distance": 22.7,                  // path distance via navmesh (if available)
    "same_groups": ["level_01"]
  }
}
```

**`path_distance` query:**
```jsonc
{
  "query": "path_distance",
  "from": "enemies/scout_02",
  "to": "player",
  "result": {
    "nav_distance": 22.7,
    "straight_distance": 15.3,
    "path_ratio": 1.48,                    // how much longer the nav path is
    "path_points": 5,                      // number of waypoints (not the points themselves)
    "traversable": true
  }
}
```

---

### 4. `spatial_inspect`

Deep inspection of a single node — all properties, children, connections, spatial context. The "tell me everything about this one thing" tool.

**When an agent should use this:** "This enemy is behaving weird, show me everything about it."

#### Parameters

```jsonc
{
  "node": {
    "type": "string",    // node path
    "required": true
  },

  // What to include — defaults to all
  "include": {
    "type": "array",
    "items": {
      "enum": ["transform", "physics", "state", "children", "signals", "script", "spatial_context"]
    },
    "default": ["transform", "physics", "state", "children", "signals", "script", "spatial_context"]
  }
}
```

#### Response Shape

```jsonc
{
  "node": "enemies/scout_02",
  "class": "CharacterBody3D",
  "instance_id": 28447,

  "transform": {
    "global_origin": [12.4, 0.0, -8.2],
    "global_rotation_deg": [0, 135, 0],
    "local_origin": [2.4, 0.0, -0.2],        // relative to parent
    "scale": [1.0, 1.0, 1.0]
  },

  "physics": {
    "velocity": [1.2, 0.0, -0.8],
    "speed": 1.44,
    "on_floor": true,
    "on_wall": false,
    "collision_layer": 1,
    "collision_mask": 3,
    "floor_normal": [0, 1, 0]
  },

  "state": {
    // All exported (@export) variables
    "exported": {
      "health": 80,
      "max_health": 100,
      "alert_level": "suspicious",
      "patrol_speed": 3.5,
      "chase_speed": 7.0,
      "detection_radius": 15.0,
      "current_target": null,
      "patrol_points": ["patrol/point_a", "patrol/point_b", "patrol/point_c"],
      "current_patrol_index": 1
    },
    // Non-exported vars the collector can see (optional, configurable)
    "internal": {
      "_time_since_last_detection": 4.2,
      "_path_recalc_timer": 0.8
    }
  },

  "children": [
    { "name": "CollisionShape3D", "class": "CollisionShape3D", "shape": "CapsuleShape3D(r=0.5, h=1.8)" },
    { "name": "Mesh", "class": "MeshInstance3D", "visible": true },
    { "name": "NavAgent", "class": "NavigationAgent3D", "target_reached": false, "distance_remaining": 12.3 },
    { "name": "DetectionArea", "class": "Area3D", "overlapping_bodies": ["player"] },
    { "name": "StateChart", "class": "Node", "script": "res://enemies/scout_state_machine.gd" }
  ],

  "signals": {
    "connected": {
      "health_changed": ["hud/enemy_health_bar:_on_health_changed"],
      "target_acquired": ["level/alert_system:_on_enemy_alert"],
      "path_completed": ["self:_on_path_completed"]
    },
    "recent_emissions": [
      { "signal": "health_changed", "frame": 2830, "args": [80] },
      { "signal": "target_acquired", "frame": 2855, "args": ["player"] }
    ]
  },

  "script": {
    "path": "res://enemies/scout_ai.gd",
    "base_class": "CharacterBody3D",
    "methods": ["_physics_process", "_on_path_completed", "take_damage", "set_alert_level"],
    "extends_chain": ["CharacterBody3D", "PhysicsBody3D", "CollisionObject3D", "Node3D", "Node"]
  },

  "spatial_context": {
    // Auto-populated — what's around this node
    "nearby_entities": [
      { "path": "enemies/guard_01", "dist": 5.2, "bearing": "left", "group": "enemies" },
      { "path": "player", "dist": 7.2, "bearing": "behind_right", "los": false },
      { "path": "walls/segment_04", "dist": 1.8, "bearing": "ahead", "type": "static" }
    ],
    "in_areas": ["patrol_zone_b", "level_01_bounds"],
    "nearest_navmesh_edge_dist": 0.3,     // useful for "why is pathfinding broken" debugging
    "camera_visible": true,
    "camera_distance": 15.3
  }
}
```

---

### 5. `spatial_watch`

Subscribe to changes on specific nodes or conditions. The server tracks these and includes them in subsequent `spatial_delta` responses even if they'd normally be filtered out.

**When an agent should use this:** "I'm about to trigger combat — watch the enemy group and tell me everything that happens."

#### Parameters

```jsonc
{
  "action": {
    "type": "string",
    "enum": ["add", "remove", "list", "clear"]
  },

  // For "add"
  "watch": {
    "type": "object",
    "optional": true,
    "properties": {
      "node": "string",              // node path or group name
      "conditions": {                 // optional — only fire on condition
        "type": "array",
        "items": {
          "property": "string",       // e.g. "health"
          "operator": "string",       // "lt", "gt", "eq", "changed"
          "value": "any"              // e.g. 20
        }
      },
      "track": {                      // what to include in deltas
        "type": "array",
        "items": { "enum": ["position", "state", "signals", "physics", "all"] },
        "default": ["all"]
      }
    }
  },

  // For "remove"
  "watch_id": {
    "type": "string",
    "optional": true
  }
}
```

#### Response

```jsonc
// "add" response
{
  "watch_id": "w_001",
  "watching": "enemies/scout_02",
  "conditions": [{ "property": "health", "operator": "lt", "value": 20 }],
  "tracking": ["all"]
}

// "list" response
{
  "watches": [
    { "id": "w_001", "node": "enemies/scout_02", "conditions": "health < 20", "tracking": "all" },
    { "id": "w_002", "node": "group:enemies", "conditions": "none", "tracking": "position, state" }
  ]
}
```

Watches appear as a dedicated section in `spatial_delta` responses:

```jsonc
{
  // ...normal delta fields...

  "watch_triggers": [
    {
      "watch_id": "w_001",
      "node": "enemies/scout_02",
      "trigger": "health dropped to 15 (was 80)",
      "frame": 2900,
      "full_state": { /* current entity state at detail:standard */ }
    }
  ]
}
```

---

### 7. `spatial_action`

Constrained game state manipulation for debugging. The agent can poke the game to reproduce bugs, test fixes, or set up observation scenarios — without needing arbitrary GDScript.

**When an agent should use this:** "Teleport the enemy to the wall so I can watch the collision." / "Pause the game so I can inspect." / "Set patrol_speed to 0."

**Design intent:** These are debugging actions, not gameplay. The tool provides safe, predictable operations that cover 90% of debugging needs. `call_method` is the escape hatch for the other 10%.

#### Parameters

```jsonc
{
  "action": {
    "type": "string",
    "enum": [
      "pause",            // pause or unpause the scene tree
      "advance_frames",   // step physics forward N frames (while paused)
      "advance_time",     // step forward N seconds of game time
      "teleport",         // move a node to a world position
      "set_property",     // change an exported variable on a node
      "emit_signal",      // fire a signal on a node with args
      "call_method",      // call a method on a node (escape hatch)
      "spawn_node",       // instantiate a scene at a position (for test setups)
      "remove_node"       // queue_free a node
    ]
  },

  // Common fields — which vary by action
  "node": { "type": "string", "optional": true },

  // pause
  "paused": { "type": "boolean", "optional": true },

  // advance_frames
  "frames": { "type": "integer", "optional": true },

  // advance_time
  "seconds": { "type": "number", "optional": true },

  // teleport — target position in world space
  "position": {
    "type": "array",    // [x, y, z] or [x, y] for 2D
    "optional": true
  },
  // Optional: also set rotation on teleport
  "rotation_deg": { "type": "number", "optional": true },  // yaw for 3D, angle for 2D

  // set_property
  "property": { "type": "string", "optional": true },
  "value": { "type": "any", "optional": true },

  // emit_signal
  "signal": { "type": "string", "optional": true },
  "args": { "type": "array", "optional": true },

  // call_method
  "method": { "type": "string", "optional": true },
  "method_args": { "type": "array", "optional": true },

  // spawn_node
  "scene_path": { "type": "string", "optional": true },    // e.g. "res://enemies/scout.tscn"
  "parent": { "type": "string", "optional": true },         // parent node path
  "name": { "type": "string", "optional": true },           // instance name

  // Whether to return a spatial_delta after the action completes.
  // Useful for "do this and show me what happened" in one round-trip.
  "return_delta": {
    "type": "boolean",
    "default": false
  }
}
```

#### Response

```jsonc
{
  "action": "teleport",
  "node": "enemies/scout_02",
  "result": "ok",
  "details": {
    "previous_position": [12.4, 0.0, -8.2],
    "new_position": [5.0, 0.0, -3.0]
  },
  "frame": 2900,

  // Present if return_delta was true
  "delta": { /* spatial_delta response shape */ }
}
```

The `return_delta` flag is key for workflow efficiency — the agent can say "teleport and show me what the scene looks like now" in a single tool call instead of two.

---

### 8. `scene_tree`

Navigate and query the Godot scene tree structure. Not spatial — this is about understanding the node hierarchy, which is Godot's core architectural primitive.

**When an agent should use this:** "Show me how this scene is organized." / "What's the parent chain for this node?" / "Find all nodes with a specific script."

#### Parameters

```jsonc
{
  "action": {
    "type": "string",
    "enum": [
      "roots",           // list root-level nodes (top of scene tree)
      "children",        // list immediate children of a node
      "subtree",         // recursive tree from a node, depth-limited
      "ancestors",       // parent chain from node to root
      "find"             // search tree by name, class, group, or script
    ]
  },

  "node": {
    "type": "string",
    "optional": true,
    "description": "Node path — required for children, subtree, ancestors"
  },

  // subtree: max depth to recurse. Default 3.
  "depth": {
    "type": "integer",
    "default": 3
  },

  // find: search criteria
  "find_by": {
    "type": "string",
    "enum": ["name", "class", "group", "script"],
    "optional": true
  },
  "find_value": {
    "type": "string",
    "optional": true
  },

  // What to include per node in results
  "include": {
    "type": "array",
    "items": { "enum": ["class", "groups", "script", "visible", "process_mode"] },
    "default": ["class", "groups"]
  }
}
```

#### Response — `subtree` example

```jsonc
{
  "root": "enemies",
  "tree": {
    "enemies": {
      "class": "Node3D",
      "groups": ["enemies_root"],
      "children": {
        "scout_02": {
          "class": "CharacterBody3D",
          "groups": ["enemies", "patrol_route_b"],
          "script": "res://enemies/scout_ai.gd",
          "children": {
            "CollisionShape3D": { "class": "CollisionShape3D" },
            "Mesh": { "class": "MeshInstance3D" },
            "NavAgent": { "class": "NavigationAgent3D" },
            "DetectionArea": { "class": "Area3D" }
          }
        },
        "guard_01": {
          "class": "CharacterBody3D",
          "groups": ["enemies", "patrol_route_a"],
          "script": "res://enemies/guard_ai.gd",
          "children": { "...": "depth_limit_reached" }
        }
      }
    }
  },
  "total_nodes": 14,
  "depth_reached": 3
}
```

#### Response — `find` example

```jsonc
{
  "find_by": "script",
  "find_value": "res://enemies/scout_ai.gd",
  "results": [
    { "path": "enemies/scout_01", "class": "CharacterBody3D", "groups": ["enemies"] },
    { "path": "enemies/scout_02", "class": "CharacterBody3D", "groups": ["enemies"] },
    { "path": "enemies/scout_03", "class": "CharacterBody3D", "groups": ["enemies"] }
  ]
}
```

---

### 9. `recording` — Collaborative Debug Sessions

The recording system enables a **human-drives, agent-observes** workflow. The human reproduces a bug while the addon captures a frame-by-frame spatial timeline. The agent then scrubs through the history to diagnose what went wrong.

This is the primary human↔agent collaboration surface. The human interacts through a Godot editor dock panel (see Human-Facing UI section below). The agent interacts through this MCP tool.

#### Parameters

```jsonc
{
  "action": {
    "type": "string",
    "enum": [
      "start",           // begin recording
      "stop",            // end recording, finalize timeline
      "status",          // check if recording, frame count, duration
      "list",            // list available recordings
      "delete",          // remove a recording

      "snapshot_at",     // get spatial state at a specific frame/time
      "query_range",     // spatial query across a frame range (temporal query)
      "find_event",      // search timeline for specific events
      "diff_frames",     // compare spatial state between two frames
      "markers",         // list human/agent markers in the timeline
      "add_marker"       // agent adds a marker to the timeline
    ]
  },

  // --- start ---
  "recording_name": {
    "type": "string",
    "optional": true,
    "description": "Human-readable label for the recording"
  },
  // What to capture. Defaults to everything dynamic.
  "capture": {
    "type": "object",
    "optional": true,
    "properties": {
      "nodes": "array | '*'",             // specific nodes or all
      "groups": "array",                  // filter by group
      "properties": "array",             // which properties per node
      "capture_interval": "integer",      // every N physics frames, default 1
      "include_signals": "boolean",       // capture signal emissions
      "include_input": "boolean",         // capture player input events
      "max_frames": "integer"             // auto-stop after N frames (safety valve)
    }
  },

  // --- snapshot_at ---
  "recording_id": {
    "type": "string",
    "optional": true,
    "description": "Which recording to query. Defaults to most recent."
  },
  // Target frame or time
  "at_frame": { "type": "integer", "optional": true },
  "at_time_ms": { "type": "integer", "optional": true },
  // Same detail/budget controls as spatial_snapshot
  "detail": { "type": "string", "optional": true },
  "token_budget": { "type": "integer", "optional": true },

  // --- query_range: temporal spatial queries ---
  "from_frame": { "type": "integer", "optional": true },
  "to_frame": { "type": "integer", "optional": true },
  "node": { "type": "string", "optional": true },
  // What to search for across the range
  "condition": {
    "type": "object",
    "optional": true,
    "properties": {
      "type": {
        "enum": [
          "proximity",       // node came within X of another node/point
          "property_change", // property crossed a threshold
          "signal_emitted",  // specific signal fired
          "entered_area",    // node entered an Area node
          "velocity_spike",  // sudden velocity change (collision indicator)
          "state_transition"  // exported var changed value
        ]
      },
      "target": "string",         // other node for proximity checks
      "threshold": "number",      // distance for proximity, value for property
      "property": "string",       // for property_change
      "signal": "string"          // for signal_emitted
    }
  },

  // --- find_event ---
  "event_type": {
    "type": "string",
    "enum": ["signal", "property_change", "collision", "area_enter", "area_exit",
             "node_added", "node_removed", "marker", "input"],
    "optional": true
  },
  "event_filter": { "type": "string", "optional": true },  // node path or signal name

  // --- diff_frames ---
  "frame_a": { "type": "integer", "optional": true },
  "frame_b": { "type": "integer", "optional": true },

  // --- add_marker ---
  "marker_label": { "type": "string", "optional": true },
  "marker_frame": { "type": "integer", "optional": true }  // defaults to current frame
}
```

#### Response Examples

**`start` response:**
```jsonc
{
  "recording_id": "rec_001",
  "name": "wall_clip_repro",
  "started_at_frame": 2800,
  "capturing": { "nodes": "*", "groups": ["enemies"], "interval": 1, "signals": true }
}
```

**`stop` response:**
```jsonc
{
  "recording_id": "rec_001",
  "name": "wall_clip_repro",
  "frames_captured": 340,
  "duration_ms": 5667,
  "frame_range": [2800, 3140],
  "nodes_tracked": 8,
  "markers": [
    { "frame": 2800, "source": "human", "label": "Starting patrol test" },
    { "frame": 3020, "source": "human", "label": "Bug happened here!" },
    { "frame": 3020, "source": "agent", "label": "velocity_spike detected on scout_02" }
  ],
  "size_estimate_kb": 420
}
```

**`snapshot_at` response:** Same shape as `spatial_snapshot` — the recording is just a queryable timeline of snapshots.

**`query_range` response — finding when proximity threshold was crossed:**
```jsonc
{
  "query": "proximity",
  "node": "enemies/scout_02",
  "target": "walls/segment_04",
  "threshold": 0.5,
  "results": [
    {
      "frame": 3012,
      "time_ms": 5200,
      "distance": 0.48,
      "node_pos": [5.1, 0.0, -3.02],
      "node_velocity": [1.2, 0.0, -0.1],
      "note": "first_breach"
    },
    {
      "frame": 3015,
      "time_ms": 5250,
      "distance": 0.12,
      "node_pos": [5.3, 0.0, -3.01],
      "node_velocity": [0.8, 0.0, 0.0],
      "note": "deepest_penetration"
    }
  ],
  "total_frames_in_range": 340,
  "frames_matching": 28
}
```

**`diff_frames` response:**
```jsonc
{
  "frame_a": 3010,
  "frame_b": 3020,
  "dt_ms": 167,
  "changes": [
    {
      "path": "enemies/scout_02",
      "position": { "a": [4.8, 0.0, -3.1], "b": [5.5, 0.0, -2.9] },
      "delta_pos": [0.7, 0.0, 0.2],
      "state": {
        "alert_level": { "a": "patrol", "b": "suspicious" }
      }
    }
  ],
  "nodes_unchanged": 6,
  "markers_between": [
    { "frame": 3020, "source": "human", "label": "Bug happened here!" }
  ]
}
```

**`markers` response:**
```jsonc
{
  "recording_id": "rec_001",
  "markers": [
    { "frame": 2800, "time_ms": 0, "source": "human", "label": "Starting patrol test" },
    { "frame": 2950, "time_ms": 2500, "source": "agent", "label": "scout_02 entered detection range" },
    { "frame": 3020, "time_ms": 3667, "source": "human", "label": "Bug happened here!" },
    { "frame": 3020, "time_ms": 3667, "source": "system", "label": "velocity_spike: scout_02 (12.4 → 0.1)" }
  ]
}
```

Note the three marker sources: **human** (from the editor UI), **agent** (via `add_marker`), and **system** (auto-detected anomalies like velocity spikes, collision events, property threshold crossings). The system markers are generated by the Rust server based on recording analysis — they're the agent's "hey something interesting happened here" breadcrumbs.

---

## Human-Facing UI — Godot Editor Dock

The GDScript addon provides a dock panel in the Godot editor for human interaction with the recording and observation system. The human doesn't need to know MCP exists — they interact with a native-feeling Godot UI.

### Dock Panel Elements

**Connection status:** Green/red indicator showing whether the Rust MCP server is connected. Port number display.

**Recording controls:**
- **Record** button (red circle) — starts a recording session. Prompts for an optional name.
- **Stop** button — ends the recording.
- **Marker** button (or keyboard shortcut, e.g. F9) — drops a timestamped marker with an optional text note. This is the primary way the human communicates with the agent during playback: "bug happened here."
- Recording timer showing elapsed time and frame count.

**Active session info:**
- Nodes being tracked (count + groups)
- Active watches (from the agent)
- Current frame number
- Memory usage estimate for the recording buffer

**Recording library:**
- List of saved recordings with name, duration, date
- Click to load for agent review
- Delete button

**Live agent activity feed (optional, low-priority):**
- Shows what the agent is querying in real-time: "Agent inspecting scout_02..." / "Agent watching enemies group..."
- Gives the human visibility into what the agent is doing without requiring them to read MCP tool calls
- Useful for building trust in the collaboration

### Keyboard Shortcuts (In-Game)

During a running game (not just in editor), the addon should support:

- **F8** — Toggle recording on/off
- **F9** — Drop a marker at current frame
- **F10** — Pause/unpause (also accessible via `spatial_action`)

These work regardless of whether the editor dock is visible, so the human can mark interesting moments during fullscreen gameplay.

### Data Flow

```
Human presses Record → Addon starts frame capture
Human plays game, presses F9 at interesting moments → Markers stored in timeline
Human presses Stop → Addon finalizes recording
Agent receives notification (or polls via recording:status)
Agent uses recording:snapshot_at, recording:query_range, recording:diff_frames
  to scrub through the timeline and diagnose the issue
Agent uses recording:add_marker to annotate its findings
Human reviews agent's markers in the dock panel
```

The recording data is stored in-memory by the addon during capture, then persisted to disk on stop (as a binary timeline file in `user://godotview_recordings/`). The Rust server reads from the addon via the TCP query protocol — the addon serves historical frames the same way it serves live frames.

---

## Resource Context — Extension to `spatial_inspect`

The `spatial_inspect` tool's `include` parameter gains a `"resources"` option that surfaces loaded asset information for a node.

```jsonc
// In spatial_inspect include options:
"include": ["transform", "physics", "state", "children", "signals", "script",
            "spatial_context", "resources"]
```

#### Resources Response Block

```jsonc
{
  // ...other inspect fields...

  "resources": {
    "mesh": {
      "resource": "res://enemies/scout_model.tres",
      "type": "ArrayMesh",
      "surface_count": 3
    },
    "material_overrides": [
      { "surface": 0, "material": "res://materials/enemy_skin.tres", "type": "StandardMaterial3D" }
    ],
    "collision_shape": {
      "resource": "CapsuleShape3D",
      "radius": 0.5,
      "height": 1.8,
      "inline": true          // created in-editor, not a .tres file
    },
    "animation_player": {
      "current_animation": "patrol_walk",
      "animations_available": ["idle", "patrol_walk", "run", "attack", "death"],
      "position_sec": 0.8,
      "length_sec": 1.2,
      "looping": true
    },
    "navigation_agent": {
      "navigation_map": "default",
      "target_position": [8.0, 0.0, -12.0],
      "path_postprocessing": "corridorfunnel"
    },
    // Shader parameters if a ShaderMaterial is present
    "shader_params": {
      "outline_color": [1, 0, 0, 1],
      "damage_flash_intensity": 0.0
    }
  }
}
```

This surfaces the information an agent needs to diagnose visual bugs ("why is this enemy invisible" → material not loaded), animation issues ("why is it T-posing" → no animation playing), and collision problems ("what shape is the collider actually").

---

## Dimension Handling — 2D / 3D Adaptation

The entire system adapts to the project's dimension context. The addon detects whether the scene root is Node2D-based or Node3D-based and reports this in connection metadata. The Rust server adjusts its representation accordingly.

### Detection

On connection, the addon reports:

```jsonc
{
  "type": "handshake",
  "godot_version": "4.3",
  "scene_dimensions": 2,    // or 3
  "physics_ticks_per_sec": 60,
  "project_name": "my_platformer"
}
```

If a scene has mixed 2D/3D content (rare but possible), the addon reports `"mixed"` and the Rust server includes both coordinate systems.

### 2D Adaptations

**Positions:** `[x, y]` instead of `[x, y, z]`. All spatial tools accept and return 2-element arrays.

**Bearings:** Simplified to 8-direction compass without elevation. No "above"/"below" — everything is on the same plane. Bearings are relative to the node's rotation (or camera orientation in camera perspective).

```jsonc
// 3D bearing
{ "dist": 7.2, "bearing": "ahead_left", "bearing_deg": 322, "elevation": "level", "occluded": false }

// 2D bearing
{ "dist": 7.2, "bearing": "ahead_left", "bearing_deg": 322, "occluded": false }
```

**Transforms:** `Transform2D` instead of `Transform3D`. Rotation is a single angle instead of euler/quaternion.

```jsonc
// 3D entity
{ "abs": [12.4, 0.0, -8.2], "rot_y": 135, "velocity": [1.2, 0.0, -0.8] }

// 2D entity
{ "abs": [12.4, -8.2], "rot": 135, "velocity": [1.2, -0.8] }
```

**Frustum → Viewport rect:** Instead of 3D camera frustum culling, 2D uses the Camera2D's visible viewport rectangle. `get_visible_nodes` returns nodes within this rect.

**Raycast:** `PhysicsRayQueryParameters2D` instead of 3D. Same tool interface, adapted internally.

**Spatial indexing:** Grid cells instead of octree. The Rust server uses a 2D spatial hash.

**Physics:** `on_floor` / `on_wall` / `on_ceiling` still present for CharacterBody2D. No `floor_normal` vector (2D normal is simpler). Collision layers/masks work identically.

### What Doesn't Change Between 2D and 3D

The MCP tool interfaces are identical — same tool names, same parameters, same response structure. The agent doesn't need to know or care whether it's debugging a 2D or 3D game. The differences are entirely in the coordinate representation within the response payloads.

The recording system, watch system, scene tree, token budgets, pagination, and action system all work identically in both dimensions.

---

## Error Handling

All tool responses include a top-level `status` field. On success it's omitted (implicit "ok"). On error:

```jsonc
{
  "error": {
    "code": "node_not_found",
    "message": "Node 'enemies/scout_99' does not exist in the scene tree",
    "suggestion": "Use scene_tree:find to search for nodes matching 'scout'"
  }
}
```

### Error Codes

| Code | Meaning | Typical Cause |
|---|---|---|
| `not_connected` | Rust server can't reach the Godot addon | Addon not running, wrong port, game not started |
| `scene_not_loaded` | Addon is connected but no scene is active | Game hasn't started, between scene transitions |
| `node_not_found` | Specified node path doesn't exist | Typo, node was freed, wrong scene |
| `invalid_cursor` | Pagination cursor is expired or invalid | Frame data was discarded, too old |
| `recording_not_found` | Referenced recording doesn't exist | Deleted, wrong ID |
| `recording_active` | Can't start recording — one is already running | Call stop first |
| `no_recording_active` | Can't stop/mark — no recording running | Call start first |
| `budget_exceeded` | Request would exceed hard cap even at minimum detail | Reduce radius or add filters |
| `method_not_found` | call_method target doesn't exist on the node | Wrong method name |
| `eval_error` | GDScript expression failed | Syntax error, runtime error |
| `timeout` | Addon didn't respond within deadline | Game frozen, heavy load, breakpoint hit |
| `dimension_mismatch` | 3D operation in 2D scene or vice versa | e.g. requesting elevation in a 2D project |

### Connection Lifecycle

```
1. Rust server starts, attempts TCP connection to addon (port 9077)
2. If addon not available: server enters "waiting" state, retries every 2s
3. On connect: addon sends handshake (version, dimensions, project info)
4. Server ACKs handshake, session begins
5. If connection drops mid-session: server enters "reconnecting" state
   - All tool calls return { error: { code: "not_connected" } }
   - Session state (watches, config) is preserved for reconnection
   - On reconnect: server re-sends watch subscriptions and config
6. Clean disconnect: server sends "disconnect" message, clears session state
```

The addon should handle the Rust server connecting/disconnecting without affecting the running game — it's purely observational unless the agent explicitly uses `spatial_action`.

---

## Addon Query Methods — Updated

Updated to include new capabilities:

| Method | Purpose | Returns |
|---|---|---|
| `get_scene_tree` | Full tree structure (paths, classes, groups) | Tree skeleton |
| `get_visible_nodes` | Nodes in camera frustum (3D) or viewport rect (2D) | Array of {path, class, position} |
| `get_near` | Nodes within radius of point | Array of {path, class, position, distance} |
| `get_node_state` | All properties of a specific node | Full property dict |
| `get_node_transform` | Transform + physics state | Transform, velocity, floor state |
| `get_node_resources` | Loaded resources (mesh, material, animations) | Resource dict |
| `get_children` | Immediate children of a node | Array of {name, class} |
| `get_ancestors` | Parent chain to root | Array of {name, class} |
| `find_nodes` | Search by name/class/group/script | Array of {path, class, groups} |
| `raycast` | Physics raycast between two points (2D or 3D) | Hit result or clear |
| `get_nav_path` | Navigation path between points | Path points, distance |
| `get_signals` | Connected signals for a node | Signal connection map |
| `subscribe_signal` | Watch for a signal emission | Ack (events pushed async) |
| `eval_expression` | Evaluate a GDScript expression | Result value |
| `get_frame_info` | Current frame number, delta, time | Frame metadata |
| `get_dimensions` | Whether scene is 2D, 3D, or mixed | Dimension info |
| `teleport_node` | Move a node to a position | Ack |
| `set_node_property` | Change a property value | Ack |
| `call_node_method` | Call a method on a node | Return value |
| `emit_node_signal` | Emit a signal on a node | Ack |
| `pause_tree` | Pause/unpause scene tree | Ack |
| `advance_physics` | Step physics N frames | Ack + new frame number |
| `spawn_node` | Instantiate a scene as child of a node | New node path |
| `remove_node` | queue_free a node | Ack |
| `recording_start` | Begin frame capture | Ack + recording ID |
| `recording_stop` | End frame capture | Recording metadata |
| `recording_frame` | Get captured frame data at index | Frame snapshot |
| `recording_query` | Query across frame range | Matching frames |
| `recording_marker` | Add/list markers | Marker data |
| `recording_list` | List saved recordings | Recording metadata array |
| `recording_delete` | Delete a saved recording | Ack |

Configure the server's behavior — what it tracks, how it categorizes nodes, what counts as "static."

**When an agent should use this:** Setup at the start of a session, or to tune the viewport for a specific debugging task.

#### Parameters

```jsonc
{
  // Nodes matching these patterns are always treated as static
  "static_patterns": {
    "type": "array",
    "items": "string",    // glob patterns: "walls/*", "terrain/*"
    "optional": true
  },

  // Properties to always include in state (by group or class)
  "state_properties": {
    "type": "object",
    "optional": true,
    "example": {
      "enemies": ["health", "alert_level", "current_target"],
      "CharacterBody3D": ["velocity"],
      "*": ["visible"]     // wildcard — all nodes
    }
  },

  // How to cluster nodes in summary view
  "cluster_by": {
    "type": "string",
    "enum": ["group", "class", "proximity", "none"],
    "default": "group"
  },

  // Bearing format preference
  "bearing_format": {
    "type": "string",
    "enum": ["cardinal", "degrees", "both"],
    "default": "both"
  },

  // Whether to include internal (non-exported) variables
  "expose_internals": {
    "type": "boolean",
    "default": false
  },

  // Physics tick polling rate (every N physics frames)
  "poll_interval": {
    "type": "integer",
    "default": 1,
    "description": "Collect data every N physics frames. Higher = less data, lower CPU."
  },

  // Hard cap on token budget for any single response.
  // Agents can request up to this via token_budget parameter.
  // Prevents context window blowouts from runaway requests.
  "token_hard_cap": {
    "type": "integer",
    "default": 5000,
    "description": "Max tokens any single tool response can produce"
  }
}
```

---

## Wire Protocol: GDScript Addon ↔ Rust Server

The GDScript addon and Rust MCP server communicate over TCP (default port 9077). The addon is the server (listens), the Rust process is the client (connects). This matches the pattern established by existing Godot MCP servers and means the addon doesn't need to know where the MCP server lives.

### Message Format

Length-prefixed JSON (4-byte big-endian length header + JSON payload). Simple, debuggable, no external dependencies in GDScript.

```
[4 bytes: payload length][JSON payload]
```

### Addon → Rust (Responses & Push Events)

The addon responds to queries and can push events for watched nodes:

```jsonc
// Response to a query
{
  "id": "req_001",           // matches the request ID
  "type": "response",
  "data": { /* query-specific payload */ }
}

// Push event (for watches / signal subscriptions)
{
  "type": "event",
  "event": "signal_emitted",
  "node": "enemies/scout_02",
  "signal": "health_changed",
  "args": [15],
  "frame": 2900
}
```

### Rust → Addon (Queries)

```jsonc
{
  "id": "req_001",
  "type": "query",
  "method": "get_visible_nodes",     // or "get_near", "raycast", "inspect_node", etc.
  "params": { /* method-specific */ }
}
```

The addon keeps things **flat and dumb** — it doesn't compute bearings, cluster nodes, or manage deltas. That's all Rust-side. The addon just efficiently answers "what does the engine say right now." See the Addon Query Methods table above for the full method list.

---

## Bearing / Relative Position System

A key part of making spatial data LLM-friendly. All relative positions are computed by the Rust server from raw transforms.

### Cardinal Bearings (8-direction)

Relative to the perspective entity's facing direction:

```
              ahead
         ahead_left  ahead_right
      left                  right
         behind_left  behind_right
              behind
```

Each bearing maps to a 45° arc. "ahead" = ±22.5° from facing direction.

### Elevation

```
"above"    — target > 2m higher
"level"    — within ±2m
"below"    — target > 2m lower
```

For significant elevation: `"above_12m"` — includes the magnitude.

### Combined Format

The `rel` block on entities uses this structure:

```jsonc
{
  "dist": 7.2,              // straight-line distance in world units
  "bearing": "ahead_left",  // 8-direction cardinal
  "bearing_deg": 322,       // exact degrees (0 = ahead, clockwise)
  "elevation": "level",     // or "above_5m", "below_2m"
  "occluded": false          // camera line-of-sight check
}
```

This gives the agent both a quick-glance spatial understanding ("ahead_left, 7m") and precise data (322°, 7.2m) when it needs to reason more carefully.

---

## Token Budget System

The server manages response size through a layered budget system. The goal: agents start cheap, expand only when needed, and the server prevents context window blowouts.

### Budget Hierarchy

1. **Detail tier** controls _what fields_ appear per entity (summary → standard → full)
2. **Token budget** controls _how many entities_ fit before truncation
3. **Hard cap** is the server-enforced ceiling (default 5000 tokens, configurable via `spatial_config`)

If the agent provides `token_budget`, the server fills up to that amount (capped by the hard cap). If omitted, the server uses detail-tier defaults:

| Tool | Detail Level | Default Budget | Hard Cap |
|---|---|---|---|
| `spatial_snapshot` | summary | 500 | 5000 |
| `spatial_snapshot` | standard | 1500 | 5000 |
| `spatial_snapshot` | full | 3000 | 5000 |
| `spatial_delta` | — | 1000 | 5000 |
| `spatial_query` | — | 500 | 2000 |
| `spatial_inspect` | — | 1500 | 3000 |
| `spatial_watch` | — | 200 | 500 |
| `spatial_config` | — | 200 | 200 |

### Pagination via Cursor

When a response is truncated, it includes a `pagination` block:

```jsonc
"pagination": {
  "truncated": true,
  "showing": 20,                      // entities in this response
  "total": 47,                        // total matching entities
  "cursor": "snap_2847_p2",           // pass to next call for more
  "omitted_nearest_dist": 15.2        // closest entity that got cut
}
```

The agent passes `cursor` back to `spatial_snapshot` to get the next page. All other parameters (perspective, radius, filters) are inherited from the original request — only `cursor` should be provided. The cursor is tied to a specific frame snapshot, so results are consistent across pages.

### Cluster Expansion

From a `summary`-tier response, the agent can drill into a specific cluster without re-querying the full scene:

```jsonc
// Agent sees cluster: { "label": "enemies", "count": 3, "summary": "2 idle, 1 patrol" }
// Agent wants more detail on just that cluster:

spatial_snapshot({ "expand": "enemies", "detail": "standard" })
```

This returns standard-tier entities for only the nodes in that cluster. The server knows what was already sent in the summary and avoids repeating perspective/frame metadata.

### Budget Strategy for Agents

The intended workflow:

1. **Start with summary** — cheap overview, see what's in the scene (~200 tokens)
2. **Expand interesting clusters** — drill into "enemies" or "physics_objects" (~400 tokens)
3. **Raise budget if needed** — `token_budget: 2000` to see more entities in one shot
4. **Cursor paginate** — only for large scenes where even a raised budget doesn't cover everything
5. **Use `spatial_inspect`** — for deep single-node investigation, separate tool entirely

This minimizes round-trips. Most debugging sessions need steps 1-2. Complex scenes might hit step 3. Cursor pagination is the escape valve, not the common path.

---

## Session State

The Rust server maintains per-session state:

- **Last snapshot frame** — for automatic delta computation
- **Spatial index** — rebuilt from addon data, supports fast nearest/radius queries
- **Static node cache** — transmitted once, suppressed in subsequent responses
- **Watch list** — active subscriptions and their conditions
- **Config** — current clustering, property, and format preferences
- **Node classification** — which nodes are static vs. dynamic (based on observation)

This state is scoped to the MCP session and discarded on disconnect.

---

## Example Agent Workflow

An agent debugging "enemies clip through walls during patrol":

```
Agent → spatial_config: {
  static_patterns: ["walls/*", "floor/*"],
  state_properties: { enemies: ["health", "alert_level", "current_patrol_index"] }
}

Agent → spatial_snapshot(detail: "summary")
  ← clusters: enemies(3), pickups(2), static_geometry(24)
  ← budget: { used: 210, limit: 500 }

Agent → spatial_snapshot(expand: "enemies", detail: "standard")
  ← 3 enemies with full spatial/state data, scout_02 heading toward wall_segment_04
  ← budget: { used: 380, limit: 1500 }

Agent → spatial_query(type: "relationship", from: "enemies/scout_02", to: "walls/segment_04")
  ← distance 1.8m, bearing "ahead" from scout, nav_distance shows path goes around

Agent → spatial_watch(add: { node: "enemies/scout_02", track: ["position", "physics"] })

Agent → [waits/triggers game advancement]

Agent → spatial_delta()
  ← scout_02 moved, now 0.3m from wall, velocity still pointing into wall
  ← budget: { used: 180, limit: 1000 }

Agent → spatial_inspect(node: "enemies/scout_02", include: ["physics", "children", "state"])
  ← collision_mask: 3, but wall collision_layer: 4 — MISMATCH FOUND
  ← NavigationAgent3D.distance_remaining: 0.1, target_reached: false — stuck on navmesh edge

Agent now has the diagnosis: collision layer mismatch + navmesh edge case.
No breakpoints needed. No code stepping. Pure spatial debugging.
Total token spend across 6 calls: ~1200 tokens of spatial data + tool call overhead.
```

### Large Scene Example (Pagination)

Agent exploring a 200-node open world scene:

```
Agent → spatial_snapshot(detail: "standard", radius: 100)
  ← 20 entities shown
  ← pagination: { truncated: true, showing: 20, total: 87, cursor: "snap_4100_p2", omitted_nearest_dist: 25.1 }

Agent → spatial_snapshot(cursor: "snap_4100_p2")
  ← next 20 entities (25-45m range)
  ← pagination: { truncated: true, showing: 20, total: 87, cursor: "snap_4100_p3" }

Agent → [decides it has enough context, stops paginating]

// OR: agent realizes it needs more in one shot next time
Agent → spatial_snapshot(detail: "standard", radius: 100, token_budget: 3000)
  ← 45 entities in a single response
  ← pagination: { truncated: true, showing: 45, total: 87, cursor: "snap_4120_p2" }
```

### Collaborative Session — Human Drives, Agent Diagnoses

The most powerful workflow: human reproduces a bug, agent analyzes the recording.

```
=== Phase 1: Human sets up the session ===

Human (in Godot editor dock): clicks "Record", names it "enemy_clip_repro"
Human (to agent): "I'm about to walk through the patrol area. The enemies
  sometimes clip through the east wall. Watch the enemies group."

Agent → recording(action: "status")
  ← recording active, id: rec_001, capturing since frame 4200

Agent → spatial_watch(add: { node: "group:enemies", track: ["position", "physics"] })
Agent → spatial_config({ state_properties: { enemies: ["health", "alert_level", "velocity"] } })

Agent: "Recording is running. I'm watching all enemies with position and
  physics tracking. Walk through the area and press F9 when you see the clip."

=== Phase 2: Human plays, marks the bug ===

Human plays through the scene for ~30 seconds.
Human sees the enemy clip through the wall.
Human presses F9, types "clipped through east wall"
Human presses F8 to stop recording.

=== Phase 3: Agent scrubs the timeline ===

Agent → recording(action: "markers", recording_id: "rec_001")
  ← markers: [
       { frame: 4200, source: "human", label: "enemy_clip_repro started" },
       { frame: 4580, source: "system", label: "velocity_spike: guard_01 (4.2 → 0.3)" },
       { frame: 4582, source: "human", label: "clipped through east wall" }
     ]

Agent → recording(action: "snapshot_at", at_frame: 4575, detail: "standard",
         groups: ["enemies"])
  ← guard_01 at [22.1, 0, -8.4], velocity [3.8, 0, -1.2], heading toward wall

Agent → recording(action: "query_range", from_frame: 4570, to_frame: 4590,
         node: "enemies/guard_01",
         condition: { type: "proximity", target: "walls/*", threshold: 0.5 })
  ← first breach at frame 4578, distance 0.48m
  ← deepest penetration at frame 4583, distance -0.12m (INSIDE wall)

Agent → recording(action: "diff_frames", frame_a: 4575, frame_b: 4585)
  ← guard_01: position delta [1.8, 0, -0.3], velocity [3.8, 0, -1.2] → [0.1, 0, 0]
  ← collision detected with walls/east_segment_07 at frame 4580 but not resolved

Agent → recording(action: "snapshot_at", at_frame: 4578)
  ← guard_01 spatial_context shows nearest_navmesh_edge_dist: 0.02

Agent → recording(action: "add_marker", marker_frame: 4578,
         marker_label: "Root cause: navmesh edge at wall + collision_mask mismatch")

Agent: "Found it. At frame 4578, guard_01 reaches the navmesh edge right at
  the east wall. The nav agent paths through because the navmesh extends slightly
  into the wall geometry. guard_01's collision_mask doesn't include layer 4 (walls),
  so physics doesn't stop it. Two fixes needed:
  1. Rebake navmesh with larger agent radius to keep edge away from walls
  2. Set guard_01 collision_mask to include layer 4
  Want me to test fix #2 right now?"


Human: "Yes, try it."

Agent -> spatial_action(action: "set_property", node: "enemies/guard_01",
         property: "collision_mask", value: 7)  // adds layer 4 (bit 3)
  <- ok, previous: 3, new: 7

Agent -> spatial_action(action: "teleport", node: "enemies/guard_01",
         position: [20.0, 0.0, -8.0])  // back to before the wall
  <- ok

Agent -> spatial_action(action: "advance_time", seconds: 5.0, return_delta: true)
  <- delta shows guard_01 patrolled toward wall, stopped at 0.5m, turned around
  <- no wall penetration

Agent: "Fix confirmed. guard_01 now stops at the wall boundary and reroutes.
  The collision_mask change to include layer 4 resolves the clipping. I've
  marked my findings in the recording timeline. You should also rebake the
  navmesh with a larger agent radius as a belt-and-suspenders fix."
```

---

## Tool Summary

| # | Tool | Purpose | Primary Use |
|---|---|---|---|
| 1 | `spatial_snapshot` | Scene overview from a perspective | "What's here right now?" |
| 2 | `spatial_delta` | What changed since last query | "What happened?" |
| 3 | `spatial_query` | Targeted spatial questions | "What's near X? Can Y see Z?" |
| 4 | `spatial_inspect` | Deep single-node investigation | "Tell me everything about this node" |
| 5 | `spatial_watch` | Subscribe to changes/conditions | "Alert me when this happens" |
| 6 | `spatial_config` | Configure tracking and display | Session setup |
| 7 | `spatial_action` | Manipulate game state for debugging | "Teleport this, change that" |
| 8 | `scene_tree` | Navigate node hierarchy | "How is this scene structured?" |
| 9 | `recording` | Capture and analyze play sessions | "Record while I reproduce this bug" |

### Architecture Summary

```
+------------------+     MCP (stdio/SSE)     +-------------------+
|  AI Agent        | <---------------------> |  Rust MCP Server  |
|  (Claude Code)   |                         |  - Semantic layer |
+------------------+                         |  - Delta engine   |
                                             |  - Spatial index  |
                                             |  - Token budget   |
                                             |  - Recording mgmt |
                                             +--------+----------+
                                                      |
                                                 TCP (port 9077)
                                                      |
                                             +--------+----------+
                                             |  GDScript Addon   |
                                             |  - Scene tree obs  |
                                             |  - Physics queries |
                                             |  - Frame capture   |
                                             |  - Signal monitor  |
                                             |  - Editor dock UI  |
                                             +-------------------+
                                                      |
                                               Godot Engine
                                              (2D or 3D scene)
```
