# Spectator — MCP Tool Contracts

Version: 1.0-draft
Protocol: MCP (Model Context Protocol)
Transport: stdio (JSON-RPC 2.0)

This document defines the complete MCP tool surface for Spectator. These 9 tools are the stable API contract between Spectator and any MCP-compatible AI agent. The tool surface is fixed — game-specific customization happens at the data layer (what properties are tracked, what nodes are grouped), not by adding or modifying tools.

---

## Tool Summary

| # | Tool | Purpose | Typical Token Cost |
|---|---|---|---|
| 1 | `spatial_snapshot` | Scene overview from a perspective | 200-1500 |
| 2 | `spatial_delta` | What changed since last query | 100-400 |
| 3 | `spatial_query` | Targeted spatial questions | 100-500 |
| 4 | `spatial_inspect` | Deep single-node investigation | 300-1500 |
| 5 | `spatial_watch` | Subscribe to changes/conditions | 50-200 |
| 6 | `spatial_config` | Configure tracking and display | 50-200 |
| 7 | `spatial_action` | Manipulate game state for debugging | 100-500 |
| 8 | `scene_tree` | Navigate node hierarchy | 200-1500 |
| 9 | `recording` | Capture and analyze play sessions | 100-1500 |

---

## Common Patterns

### Error Responses

All tools may return errors instead of normal responses:

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

| Code | Meaning |
|---|---|
| `not_connected` | MCP server can't reach the Godot addon (addon not running, game not started) |
| `scene_not_loaded` | Addon connected but no scene is active (between scene transitions) |
| `node_not_found` | Specified node path doesn't exist |
| `invalid_cursor` | Pagination cursor is expired or invalid |
| `recording_not_found` | Referenced recording ID doesn't exist |
| `recording_active` | Can't start recording — one is already running |
| `no_recording_active` | Can't stop/mark — no recording running |
| `budget_exceeded` | Request would exceed hard cap even at minimum detail |
| `method_not_found` | call_method target doesn't exist on the node |
| `eval_error` | GDScript expression evaluation failed |
| `timeout` | Addon didn't respond within deadline (game frozen, breakpoint hit) |
| `dimension_mismatch` | 3D operation in 2D scene or vice versa |

### Token Budget

Every response includes a `budget` block:

```jsonc
"budget": {
  "used": 280,        // approximate tokens in this response
  "limit": 500,       // effective budget for this call
  "hard_cap": 5000    // server maximum (configurable via spatial_config)
}
```

The `token_budget` parameter (available on snapshot, delta, query, inspect, recording tools) sets the target budget for a single response. The server fills up to that amount (capped by `hard_cap`). If omitted, detail-tier defaults apply.

### Pagination

When a response is truncated to fit the budget, a `pagination` block is included:

```jsonc
"pagination": {
  "truncated": true,
  "showing": 20,
  "total": 47,
  "cursor": "snap_2847_p2",
  "omitted_nearest_dist": 15.2
}
```

Pass `cursor` back to the same tool to get the next page. All other parameters are inherited from the original request. The cursor is tied to a specific frame snapshot for consistency across pages.

### Coordinate Format

Positions are `[x, y, z]` in 3D scenes, `[x, y]` in 2D scenes. All coordinates are in Godot world units. The MCP tools use the same format regardless of dimension — the array length indicates the scene type.

---

## Tool 1: `spatial_snapshot`

The primary view. Returns a token-budgeted representation of the scene from a spatial perspective.

**When to use:** "What does the scene look like right now?" — the spatial equivalent of opening a file.

### Parameters

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
    "type": "array",   // [x, y, z] or [x, y]
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

  // Soft token budget. Controls how many entities are included before
  // truncation. Server enforces a hard cap (default 5000, configurable
  // via spatial_config).
  "token_budget": {
    "type": "integer",
    "optional": true
  },

  // Continuation cursor from a previous truncated response.
  // When present, returns the next page of entities.
  // All other parameters are inherited from the original request.
  "cursor": {
    "type": "string",
    "optional": true
  },

  // Expand a specific cluster from a previous summary response.
  // Returns standard/full detail for just that cluster's entities.
  "expand": {
    "type": "string",
    "optional": true
  }
}
```

### Response — `detail: "summary"` (~150-300 tokens)

```jsonc
{
  "frame": 2847,
  "timestamp_ms": 47450,
  "perspective": {
    "position": [0.0, 1.8, 0.0],
    "facing": "north",
    "facing_deg": 5.2
  },

  "clusters": [
    {
      "label": "enemies",
      "count": 3,
      "nearest": { "node": "enemy/scout_02", "dist": 7.2, "bearing": "ahead_left" },
      "farthest_dist": 22.1,
      "summary": "2 idle, 1 patrol"
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
      "note": "unchanged"
    }
  ],

  "recent_events": [
    { "frame": 2840, "event": "enemy/guard_01 entered area 'patrol_zone_b'" },
    { "frame": 2845, "event": "pickups/ammo_03 removed (collected)" }
  ],

  "total_nodes_tracked": 29,
  "total_nodes_visible": 14,

  "pagination": {                    // omitted if not truncated
    "truncated": true,
    "showing": 14,
    "total": 29,
    "cursor": "snap_2847_p2",
    "omitted_nearest_dist": 15.2
  },

  "budget": {
    "used": 280,
    "limit": 500,
    "hard_cap": 5000
  }
}
```

### Response — `detail: "standard"` (~400-800 tokens)

Everything in summary, plus individual entries for dynamic nodes:

```jsonc
{
  // ...summary fields (frame, timestamp, perspective, clusters, recent_events)...

  "entities": [
    {
      "path": "enemies/scout_02",
      "class": "CharacterBody3D",
      "rel": {
        "dist": 7.2,
        "bearing": "ahead_left",
        "bearing_deg": 322,
        "elevation": "level",
        "occluded": false
      },
      "abs": [12.4, 0.0, -8.2],
      "rot_y": 135,
      "velocity": [1.2, 0.0, -0.8],     // only if moving
      "groups": ["enemies", "patrol_route_b"],
      "state": {
        "health": 80,
        "alert_level": "suspicious",
        "current_target": null
      },
      "signals_recent": [
        { "signal": "health_changed", "frame": 2830 }
      ]
    }
    // ...more entities, sorted by distance (nearest first)
  ],

  "static_summary": {
    "count": 24,
    "categories": {
      "wall_segments": 12,
      "props": 8,
      "lights": 4
    }
  },

  "pagination": { /* ... */ },
  "budget": { /* ... */ }
}
```

### Response — `detail: "full"` (~1000+ tokens)

Adds to standard:

```jsonc
{
  // ...standard fields...

  "entities": [
    {
      // ...standard entity fields...

      "transform": {
        "origin": [12.4, 0.0, -8.2],
        "basis": [[1,0,0],[0,1,0],[0,0,1]],
        "scale": [1.0, 1.0, 1.0]
      },
      "physics": {
        "velocity": [1.2, 0.0, -0.8],
        "on_floor": true,
        "collision_layer": 1,
        "collision_mask": 3
      },
      "children": [
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

  "static_nodes": [
    { "path": "walls/segment_01", "class": "StaticBody3D", "pos": [0, 0, -5], "aabb": [4, 3, 0.5] }
    // ...
  ]
}
```

---

## Tool 2: `spatial_delta`

Returns only what changed since the last query. Designed for agent loops: take an action, then check what happened.

**When to use:** "I just told the enemy to move — did it? What changed?"

### Parameters

```jsonc
{
  // Frame to diff against. If omitted, diffs against the last query.
  "since_frame": {
    "type": "integer",
    "optional": true
  },

  // Same perspective/radius/filter options as spatial_snapshot
  "perspective": { "type": "string", "default": "camera" },
  "radius": { "type": "number", "default": 50.0 },
  "groups": { "type": "array", "optional": true },
  "class_filter": { "type": "array", "optional": true },

  "token_budget": { "type": "integer", "optional": true }
}
```

### Response (~100-400 tokens)

```jsonc
{
  "from_frame": 2847,
  "to_frame": 2863,
  "dt_ms": 267,

  "moved": [
    {
      "path": "enemies/scout_02",
      "pos": [13.1, 0.0, -9.0],
      "delta_pos": [0.7, 0.0, -0.8],
      "dist_to_focal": 8.4
    }
  ],

  "state_changed": [
    {
      "path": "enemies/scout_02",
      "changes": { "alert_level": ["suspicious", "alert"] }   // [old, new]
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

  "static_changed": false,

  // Present if watches triggered
  "watch_triggers": [
    {
      "watch_id": "w_001",
      "node": "enemies/scout_02",
      "trigger": "health dropped to 15 (was 80)",
      "frame": 2900,
      "full_state": { /* entity state at standard detail */ }
    }
  ],

  "budget": { /* ... */ }
}
```

---

## Tool 3: `spatial_query`

Targeted spatial questions. Instead of fetching the whole scene and filtering, the agent asks a specific question.

**When to use:** "What's near the player?" / "Is there line of sight between A and B?" / "What's in this area?"

### Parameters

```jsonc
{
  "query_type": {
    "type": "string",
    "enum": ["nearest", "radius", "raycast", "area", "path_distance", "relationship"]
  },

  // Origin — node path or world position
  "from": {
    "type": "string | array",   // "player" or [10, 0, 5]
    "required": true
  },

  // Target — for raycast and relationship queries
  "to": {
    "type": "string | array",
    "optional": true
  },

  // For nearest queries
  "k": { "type": "integer", "default": 5 },

  // For radius/area queries
  "radius": { "type": "number", "default": 20.0 },

  // Filters
  "groups": { "type": "array", "optional": true },
  "class_filter": { "type": "array", "optional": true }
}
```

### Response — `nearest`

```jsonc
{
  "query": "nearest",
  "from": "player",
  "results": [
    { "path": "pickups/health_01", "dist": 3.1, "bearing": "right", "class": "Area3D" },
    { "path": "enemies/scout_02", "dist": 7.2, "bearing": "ahead_left", "class": "CharacterBody3D" },
    { "path": "props/barrel_07", "dist": 8.9, "bearing": "behind_right", "class": "StaticBody3D" }
  ],
  "budget": { /* ... */ }
}
```

### Response — `raycast`

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
  },
  "budget": { /* ... */ }
}
```

### Response — `relationship`

```jsonc
{
  "query": "relationship",
  "from": "enemies/scout_02",
  "to": "player",
  "result": {
    "distance": 15.3,
    "bearing_from_a": "behind_right",
    "bearing_from_b": "ahead_left",
    "elevation_diff": 0.0,
    "line_of_sight": false,
    "occluder": "walls/segment_04",
    "nav_distance": 22.7,
    "same_groups": ["level_01"]
  },
  "budget": { /* ... */ }
}
```

### Response — `path_distance`

```jsonc
{
  "query": "path_distance",
  "from": "enemies/scout_02",
  "to": "player",
  "result": {
    "nav_distance": 22.7,
    "straight_distance": 15.3,
    "path_ratio": 1.48,
    "path_points": 5,
    "traversable": true
  },
  "budget": { /* ... */ }
}
```

### Response — `radius`

```jsonc
{
  "query": "radius",
  "from": "player",
  "radius": 15.0,
  "results": [
    { "path": "pickups/health_01", "dist": 3.1, "bearing": "right", "class": "Area3D" },
    { "path": "enemies/scout_02", "dist": 7.2, "bearing": "ahead_left", "class": "CharacterBody3D" },
    { "path": "props/barrel_07", "dist": 8.9, "bearing": "behind_right", "class": "StaticBody3D" },
    { "path": "enemies/guard_01", "dist": 12.1, "bearing": "left", "class": "CharacterBody3D" }
  ],
  "budget": { /* ... */ }
}
```

---

## Tool 4: `spatial_inspect`

Deep inspection of a single node — all properties, children, connections, spatial context. The "tell me everything about this one thing" tool.

**When to use:** "This enemy is behaving weird, show me everything about it."

### Parameters

```jsonc
{
  "node": {
    "type": "string",
    "required": true
  },

  "include": {
    "type": "array",
    "items": {
      "enum": ["transform", "physics", "state", "children", "signals", "script",
               "spatial_context", "resources"]
    },
    "default": ["transform", "physics", "state", "children", "signals", "script", "spatial_context"]
  }
}
```

### Response

```jsonc
{
  "node": "enemies/scout_02",
  "class": "CharacterBody3D",
  "instance_id": 28447,

  "transform": {
    "global_origin": [12.4, 0.0, -8.2],
    "global_rotation_deg": [0, 135, 0],
    "local_origin": [2.4, 0.0, -0.2],
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
    "internal": {                                // only if expose_internals is true
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
    "nearby_entities": [
      { "path": "enemies/guard_01", "dist": 5.2, "bearing": "left", "group": "enemies" },
      { "path": "player", "dist": 7.2, "bearing": "behind_right", "los": false },
      { "path": "walls/segment_04", "dist": 1.8, "bearing": "ahead", "type": "static" }
    ],
    "in_areas": ["patrol_zone_b", "level_01_bounds"],
    "nearest_navmesh_edge_dist": 0.3,
    "camera_visible": true,
    "camera_distance": 15.3
  },

  "resources": {                               // only if "resources" in include
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
      "inline": true
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
    "shader_params": {
      "outline_color": [1, 0, 0, 1],
      "damage_flash_intensity": 0.0
    }
  },

  "budget": { /* ... */ }
}
```

---

## Tool 5: `spatial_watch`

Subscribe to changes on specific nodes or conditions. The server tracks these and includes them in subsequent `spatial_delta` responses even if they'd normally be filtered out.

**When to use:** "I'm about to trigger combat — watch the enemy group and tell me everything that happens."

### Parameters

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
      "node": "string",                // node path or "group:group_name"
      "conditions": {
        "type": "array",
        "items": {
          "property": "string",
          "operator": "string",        // "lt", "gt", "eq", "changed"
          "value": "any"
        }
      },
      "track": {
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

### Responses

```jsonc
// "add"
{
  "watch_id": "w_001",
  "watching": "enemies/scout_02",
  "conditions": [{ "property": "health", "operator": "lt", "value": 20 }],
  "tracking": ["all"],
  "budget": { /* ... */ }
}

// "list"
{
  "watches": [
    { "id": "w_001", "node": "enemies/scout_02", "conditions": "health < 20", "tracking": "all" },
    { "id": "w_002", "node": "group:enemies", "conditions": "none", "tracking": "position, state" }
  ],
  "budget": { /* ... */ }
}

// "remove" / "clear"
{
  "result": "ok",
  "removed": 1,             // number of watches removed
  "budget": { /* ... */ }
}
```

Watch triggers appear in `spatial_delta` responses — see Tool 2.

---

## Tool 6: `spatial_config`

Configure the server's behavior — what it tracks, how it categorizes nodes, what counts as "static."

**When to use:** Setup at the start of a session, or to tune for a specific debugging task.

### Parameters

```jsonc
{
  // Nodes matching these patterns are always treated as static
  "static_patterns": {
    "type": "array",
    "items": "string",       // glob patterns: "walls/*", "terrain/*"
    "optional": true
  },

  // Properties to always include in state (by group or class)
  "state_properties": {
    "type": "object",
    "optional": true,
    "example": {
      "enemies": ["health", "alert_level", "current_target"],
      "CharacterBody3D": ["velocity"],
      "*": ["visible"]
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
    "default": 1
  },

  // Hard cap on token budget for any single response
  "token_hard_cap": {
    "type": "integer",
    "default": 5000
  }
}
```

### Response

```jsonc
{
  "result": "ok",
  "config": {
    "static_patterns": ["walls/*", "terrain/*"],
    "state_properties": { "enemies": ["health", "alert_level"] },
    "cluster_by": "group",
    "bearing_format": "both",
    "expose_internals": false,
    "poll_interval": 1,
    "token_hard_cap": 5000
  },
  "budget": { /* ... */ }
}
```

---

## Tool 7: `spatial_action`

Constrained game state manipulation for debugging. The agent can poke the game to reproduce bugs, test fixes, or set up observation scenarios.

**When to use:** "Teleport the enemy to the wall so I can watch the collision." / "Pause the game." / "Set patrol_speed to 0."

### Parameters

```jsonc
{
  "action": {
    "type": "string",
    "enum": [
      "pause",
      "advance_frames",
      "advance_time",
      "teleport",
      "set_property",
      "emit_signal",
      "call_method",
      "spawn_node",
      "remove_node"
    ]
  },

  "node": { "type": "string", "optional": true },

  // pause
  "paused": { "type": "boolean", "optional": true },

  // advance_frames
  "frames": { "type": "integer", "optional": true },

  // advance_time
  "seconds": { "type": "number", "optional": true },

  // teleport
  "position": { "type": "array", "optional": true },        // [x, y, z] or [x, y]
  "rotation_deg": { "type": "number", "optional": true },

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
  "scene_path": { "type": "string", "optional": true },
  "parent": { "type": "string", "optional": true },
  "name": { "type": "string", "optional": true },

  // Whether to return a spatial_delta after the action completes
  "return_delta": {
    "type": "boolean",
    "default": false
  }
}
```

### Response

```jsonc
// teleport example
{
  "action": "teleport",
  "node": "enemies/scout_02",
  "result": "ok",
  "details": {
    "previous_position": [12.4, 0.0, -8.2],
    "new_position": [5.0, 0.0, -3.0]
  },
  "frame": 2900,
  "delta": { /* spatial_delta response — present if return_delta was true */ },
  "budget": { /* ... */ }
}

// set_property example
{
  "action": "set_property",
  "node": "enemies/scout_02",
  "result": "ok",
  "details": {
    "property": "collision_mask",
    "previous_value": 3,
    "new_value": 7
  },
  "frame": 2901,
  "budget": { /* ... */ }
}

// pause example
{
  "action": "pause",
  "result": "ok",
  "details": {
    "paused": true
  },
  "frame": 2902,
  "budget": { /* ... */ }
}

// call_method example
{
  "action": "call_method",
  "node": "enemies/scout_02",
  "result": "ok",
  "details": {
    "method": "take_damage",
    "return_value": null
  },
  "frame": 2903,
  "budget": { /* ... */ }
}
```

---

## Tool 8: `scene_tree`

Navigate and query the Godot scene tree structure. Not spatial — this is about understanding the node hierarchy.

**When to use:** "Show me how this scene is organized." / "Find all nodes with a specific script."

### Parameters

```jsonc
{
  "action": {
    "type": "string",
    "enum": ["roots", "children", "subtree", "ancestors", "find"]
  },

  "node": {
    "type": "string",
    "optional": true,
    "description": "Node path — required for children, subtree, ancestors"
  },

  // subtree: max depth to recurse
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

### Response — `subtree`

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
  "depth_reached": 3,
  "budget": { /* ... */ }
}
```

### Response — `find`

```jsonc
{
  "find_by": "script",
  "find_value": "res://enemies/scout_ai.gd",
  "results": [
    { "path": "enemies/scout_01", "class": "CharacterBody3D", "groups": ["enemies"] },
    { "path": "enemies/scout_02", "class": "CharacterBody3D", "groups": ["enemies"] },
    { "path": "enemies/scout_03", "class": "CharacterBody3D", "groups": ["enemies"] }
  ],
  "budget": { /* ... */ }
}
```

### Response — `roots`

```jsonc
{
  "roots": [
    { "name": "Main", "class": "Node3D", "groups": [] },
    { "name": "SpectatorRuntime", "class": "Node", "groups": ["spectator_internal"] }
  ],
  "budget": { /* ... */ }
}
```

### Response — `ancestors`

```jsonc
{
  "node": "enemies/scout_02/NavAgent",
  "ancestors": [
    { "name": "NavAgent", "class": "NavigationAgent3D", "groups": [] },
    { "name": "scout_02", "class": "CharacterBody3D", "groups": ["enemies"] },
    { "name": "enemies", "class": "Node3D", "groups": ["enemies_root"] },
    { "name": "Main", "class": "Node3D", "groups": [] }
  ],
  "budget": { /* ... */ }
}
```

---

## Tool 9: `recording`

Capture and analyze play sessions. The human drives, the agent observes. The recording system captures a frame-by-frame spatial timeline that the agent can scrub through to diagnose issues.

**When to use:** "Record while I reproduce this bug." / "What happened at frame 3020?" / "When did the enemy first get close to the wall?"

### Parameters

```jsonc
{
  "action": {
    "type": "string",
    "enum": [
      "start",            // begin recording
      "stop",             // end recording
      "status",           // check recording state
      "list",             // list saved recordings
      "delete",           // remove a recording
      "snapshot_at",      // spatial state at a specific frame
      "query_range",      // search across frame range
      "trajectory",       // position/property timeseries across frame range
      "find_event",       // search for specific events
      "diff_frames",      // compare two frames
      "markers",          // list markers in a recording
      "add_marker"        // agent adds a marker
    ]
  },

  // --- start ---
  "recording_name": { "type": "string", "optional": true },
  "capture": {
    "type": "object",
    "optional": true,
    "properties": {
      "nodes": "array | '*'",
      "groups": "array",
      "properties": "array",
      "capture_interval": "integer",
      "include_signals": "boolean",
      "include_input": "boolean",
      "max_frames": "integer"
    }
  },

  // --- snapshot_at ---
  "recording_id": { "type": "string", "optional": true },
  "at_frame": { "type": "integer", "optional": true },
  "at_time_ms": { "type": "integer", "optional": true },
  "detail": { "type": "string", "optional": true },
  "token_budget": { "type": "integer", "optional": true },

  // --- query_range ---
  "from_frame": { "type": "integer", "optional": true },
  "to_frame": { "type": "integer", "optional": true },
  "node": { "type": "string", "optional": true },
  "condition": {
    "type": "object",
    "optional": true,
    "properties": {
      "type": {
        "enum": ["proximity", "property_change", "signal_emitted",
                 "entered_area", "velocity_spike", "state_transition",
                 "collision", "moved"]
      },
      "target": "string",
      "threshold": "number",
      "property": "string",
      "signal": "string"
    }
  },

  // --- find_event ---
  "event_type": {
    "type": "string",
    "enum": ["signal", "property_change", "collision", "area_enter", "area_exit",
             "node_added", "node_removed", "marker", "input"],
    "optional": true
  },
  "event_filter": { "type": "string", "optional": true },

  // --- diff_frames ---
  "frame_a": { "type": "integer", "optional": true },
  "frame_b": { "type": "integer", "optional": true },

  // --- add_marker ---
  "marker_label": { "type": "string", "optional": true },
  "marker_frame": { "type": "integer", "optional": true }
}
```

### Response — `start`

```jsonc
{
  "recording_id": "rec_001",
  "name": "wall_clip_repro",
  "started_at_frame": 2800,
  "capturing": { "nodes": "*", "groups": ["enemies"], "interval": 1, "signals": true },
  "budget": { /* ... */ }
}
```

### Response — `stop`

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
    { "frame": 3020, "source": "system", "label": "velocity_spike detected on scout_02" }
  ],
  "size_kb": 420,
  "budget": { /* ... */ }
}
```

### Response — `snapshot_at`

Same shape as `spatial_snapshot` response (the recording is a queryable timeline of snapshots).

### Response — `trajectory`

```jsonc
{
  "node": "Camera3D",
  "from_frame": 100,
  "to_frame": 300,
  "sample_interval": 10,
  "samples": [
    {"frame": 100, "time_ms": 1667, "position": [0, 60, 60]},
    {"frame": 110, "time_ms": 1833, "position": [0, 54, 54]},
    {"frame": 120, "time_ms": 2000, "position": [0, 48, 48]}
  ],
  "total_frames_in_range": 200,
  "samples_returned": 21,
  "budget": { /* ... */ }
}
```

### Response — `query_range`

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
  "frames_matching": 28,
  "budget": { /* ... */ }
}
```

### Response — `diff_frames`

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
  ],
  "budget": { /* ... */ }
}
```

### Response — `markers`

```jsonc
{
  "recording_id": "rec_001",
  "markers": [
    { "frame": 2800, "time_ms": 0, "source": "human", "label": "Starting patrol test" },
    { "frame": 2950, "time_ms": 2500, "source": "agent", "label": "scout_02 entered detection range" },
    { "frame": 3020, "time_ms": 3667, "source": "human", "label": "Bug happened here!" },
    { "frame": 3020, "time_ms": 3667, "source": "system", "label": "velocity_spike: scout_02 (12.4 -> 0.1)" }
  ],
  "budget": { /* ... */ }
}
```

Marker sources:
- **human**: From the editor dock or F9 keyboard shortcut
- **agent**: Via `recording(action: "add_marker")`
- **code**: Via `SpectatorRuntime.marker("label")` from game scripts (system tier by default, rate-limited; supports `"deliberate"` and `"silent"` tiers)
- **system**: Auto-detected anomalies (velocity spikes, collision events, property threshold crossings)

### Response — `list`

```jsonc
{
  "recordings": [
    {
      "id": "rec_001",
      "name": "wall_clip_repro",
      "duration_ms": 5667,
      "frames": 340,
      "frame_range": [2800, 3140],
      "nodes_tracked": 8,
      "markers_count": 4,
      "size_kb": 420,
      "created_at": "2026-03-05T14:30:00Z"
    }
  ],
  "budget": { /* ... */ }
}
```

### Response — `find_event`

```jsonc
{
  "recording_id": "rec_001",
  "event_type": "signal",
  "filter": "health_changed",
  "events": [
    { "frame": 2830, "time_ms": 500, "node": "enemies/scout_02", "signal": "health_changed", "args": [80] },
    { "frame": 2900, "time_ms": 1667, "node": "enemies/scout_02", "signal": "health_changed", "args": [15] }
  ],
  "budget": { /* ... */ }
}
```

### Response — `status`

```jsonc
{
  "recording_active": true,
  "recording_id": "rec_001",
  "name": "wall_clip_repro",
  "frames_captured": 180,
  "duration_ms": 3000,
  "nodes_tracked": 8,
  "markers_count": 1,
  "buffer_size_kb": 220
}
```

---

## Agent Workflow Patterns

### Pattern 1: Quick Scene Assessment

```
1. spatial_snapshot(detail: "summary")           → 200 tokens, scene overview
2. spatial_snapshot(expand: "enemies")            → 400 tokens, enemy details
3. spatial_inspect(node: "enemies/scout_02")      → 800 tokens, deep dive on one
Total: ~1400 tokens for complete understanding
```

### Pattern 2: Observe → Act → Verify

```
1. spatial_snapshot(detail: "standard")           → scene state before
2. spatial_action(action: "teleport", ...,
      return_delta: true)                         → action + immediate delta
Total: 2 calls instead of 3 (snapshot + action + delta)
```

### Pattern 3: Recording Analysis

```
1. recording(action: "markers")                   → find human markers
2. recording(action: "snapshot_at", at_frame: N)  → state at marked moment
3. recording(action: "query_range", ...)          → search for the anomaly
4. recording(action: "diff_frames", ...)          → compare before/after
5. recording(action: "add_marker", ...)           → annotate findings
Total: 5 calls for full timeline diagnosis
```

### Pattern 4: Live Monitoring

```
1. spatial_config(state_properties: {...})        → set up tracking
2. spatial_watch(add: { node: "group:enemies" })  → subscribe to changes
3. [agent waits or advances time]
4. spatial_delta()                                → see changes + watch triggers
5. [repeat 3-4]
```
