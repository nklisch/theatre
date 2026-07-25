# Stage — MCP Tool Contracts

Version: 1.0-draft
Protocol: MCP (Model Context Protocol)
Transport: stdio (JSON-RPC 2.0)

This document describes the current MCP tool surface for Stage as implemented — the contract between Stage and any MCP-compatible AI agent. Code is the source of truth; where this document and the implementation disagree, the implementation wins and this document should be corrected. The tool surface is project-owned and changes in place as the design improves (see the change-in-place principle in `docs/PRINCIPLES.md`); agents calling these tools adapt on each session, so no stability guarantees or versioned schemas are maintained here. Game-specific customization happens at the data layer (what properties are tracked, what nodes are grouped) rather than through per-game tool variants.

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
| 9 | `clips` | Always-on dashcam capture, clip analysis, visual artifacts | 100-1500 |

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
| `recording_not_found` | Referenced clip ID doesn't exist |
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

The `token_budget` parameter (available on snapshot, delta, query, inspect, and clips tools) sets the target budget for a single response. The server fills up to that amount (capped by `hard_cap`). If omitted, detail-tier defaults apply.

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
    { "name": "StageRuntime", "class": "Node", "groups": ["stage_internal"] }
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

## Tool 9: `clips`

Always-on dashcam capture and clip analysis. The addon continuously buffers spatial frames (every physics frame) and viewport screenshots (every 4 physics frames by default) in memory; markers and `save` flush windows of that buffer to per-clip SQLite databases that the agent then scrubs, diffs, queries, and renders visual artifacts from. There is no start/stop: capture is always running while the addon is active.

Marker sources: **human** (dock / F9), **agent** (`add_marker`), **code** (`Stage.marker()` from game scripts), **system** (auto-detected anomalies — velocity spikes and visual anomalies). System and deliberate tiers have independent pre/post capture windows and rate limiting.

### Actions

| Action | Purpose | Required | Optional |
|---|---|---|---|
| `add_marker` | Mark the current moment (triggers clip save) | — | `marker_label`, `marker_frame` |
| `save` | Force-save the dashcam buffer as a clip | — | `marker_label` |
| `status` | Dashcam state, config, capture probe, anomaly detector, screenshot ring | — | — |
| `list` | List saved clips with metadata | — | — |
| `delete` | Remove a clip | `clip_id` | — |
| `markers` | List markers in a clip | — | `clip_id` |
| `snapshot_at` | Spatial state at a frame | `at_frame` or `at_time_ms` | `clip_id`, `detail`, `token_budget` |
| `trajectory` | Position/property timeseries | `node`, `from_frame`, `to_frame` | `clip_id`, `properties`, `sample_interval` |
| `query_range` | Frames matching a condition | `node`, `from_frame`, `to_frame`, `condition` | `clip_id` |
| `diff_frames` | Compare two frames | `frame_a`, `frame_b` | `clip_id` |
| `find_event` | Search clip events | `event_type` | `event_filter`, `clip_id` |
| `screenshot_at` | Viewport JPEG nearest a frame/time as MCP image content | `at_frame` or `at_time_ms` | `clip_id` |
| `screenshots` | Screenshot metadata for a clip (no image data) | — | `clip_id` |
| `visual_artifact` | Generate a temporal visual artifact (below) | `artifact` | `clip_id`, `at_frame`, `at_time_ms`, `reference_frame`, `tile_limit`, `inline_image`, `token_budget`, `node`, `crop_fraction` |
| `config` | Forward dashcam config JSON to the addon | `config` (object) | — |

`clip_id` defaults to the most recent clip. `condition` (query_range) is an object with a `type` key: `moved`, `proximity` (`target`, `threshold`), `velocity_spike` (`threshold`), `property_change` (`property`), `state_transition` (`property`), `signal_emitted` (`signal`), `entered_area`, `collision`.

### Response — `status`

```jsonc
{
  "dashcam_state": "buffering",          // buffering | post_capture | disabled
  "buffer_frames": 1240, "buffer_kb": 2480,
  "config": { /* effective dashcam config, incl. anomaly_* fields */ },
  "capture_probe": {
    "readback_ms_ema": 1.69, "readback_ms_max": 4.39,
    "dispatched": 1322, "dropped_queue_full": 0, "encode_depth_max": 1,
    "physics_delta_ms_ema": 16.6, "physics_delta_ms_p95_window": 28.5,
    "analysis_ms_ema": 0.21, "analysis_ms_max": 0.83
  },
  "screenshot_buffer_count": 300, "screenshot_buffer_kb": 6200,
  "screenshots_available": true,
  "screenshot_gaps": { "count": 0, "dropped": 0, "overflow": 0 },
  "anomaly": {
    "active": true, "reason": "",       // reason set when inactive: editor_hint | screenshots_disabled | anomaly_disabled | dashcam_disabled
    "frames_analyzed": 4120, "frames_skipped": 3,
    "metric_ema": 0.031, "last_proportion": 0.028,
    "anomalous_streak": 0,
    "triggers_total": 1, "suppressed_cooldown": 2,
    "last_trigger_frame": 182340, "last_trigger_proportion": 0.47
  },
  "budget": { /* ... */ }
}
```

### Response — `visual_artifact`

Two content blocks: a compact JSON manifest (text) and, unless `inline_image: false`, the rendered PNG (image). `artifact` kinds:

- `storyboard` — 3–12 informative frames (change-scored selection with stated reasons) as one labeled montage. `tile_limit` (3–12, default 8), anchor via `at_frame`/`at_time_ms` (default: first human/agent marker, else clip midpoint).
- `motion_history` — single recency-decayed image of where movement happened. `reference_frame` for the backdrop (default: anchor).
- `difference_map` — change frequency + timing heatmaps relative to `reference_frame` (default: clip start).
- `node_filmstrip` — fixed-size crop following a node's projected screen position across the clip. Requires `node` (exact path). `crop_fraction` (default 0.25, clamped 0.05–1.0). Requires camera data (clips recorded by addon ≥ camera-capture version); 3D scenes only. Manifest carries per-tile statuses (`on_screen`, `off_screen`, `behind_camera`, `node_absent`, `camera_absent`), projection counts, and `camera_switches`.

```jsonc
{
  "clip_id": "clip_1a2b3c4d", "kind": "storyboard", "anchor_frame": 1234,
  "frames_analyzed": 217, "subsampled": false,
  "cadence": { "interval_frames": 4, "captured": 217, "dropped": 3,
               "coverage": { "first_frame": 1200, "last_frame": 2068 } },
  "gaps": [ { "start_frame": 1400, "end_frame": 1416, "reason": "encode_queue_full", "dropped": 3 } ],
  "selected_frames": [ { "frame": 1200, "timestamp_ms": 81200, "reasons": ["pre_anchor"] } ],
  "image": { "width": 1920, "height": 270, "bytes": 310442, "sha256": "…", "cache": "generated" },
  "budget": { "used": 280, "limit": 1500, "hard_cap": 5000 }
}
```

Artifacts are deterministic and cached in the clip database (content-addressed by kind + params + clip fingerprint + generator version); repeat calls return `cache: "hit"` with identical bytes. Degradation conditions return content-level JSON instead of an image: `no_screenshots`, `insufficient_frames`, `decode_failed`, `generation_failed`, plus `no_camera_data`, `unsupported_scene_2d`, `node_not_found` (with `sample_paths`), `unsupported_projection` for `node_filmstrip`.

### Response — `config`

Echoes the applied dashcam config. Tunable keys include screenshot cadence (`screenshot_interval_frames`, `screenshot_quality`, `screenshot_max_dimension`, `screenshot_byte_cap_mb`), dense burst mode (`dense_burst_*`), and the visual anomaly detector (`anomaly_enabled`, `anomaly_min_proportion`, `anomaly_relative_factor`, `anomaly_sustained_frames`, `anomaly_cooldown_sec`, `anomaly_noise_floor`). A project's `stage.toml [dashcam]` section (when present) is pushed after handshake and takes precedence on each new connection; runtime `config` calls apply until then.

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

### Pattern 3: Clip Analysis

```
1. clips(action: "markers")                       → find human markers
2. clips(action: "snapshot_at", at_frame: N)      → state at marked moment
3. clips(action: "query_range", ...)              → search for the anomaly
4. clips(action: "diff_frames", ...)              → compare before/after
5. clips(action: "visual_artifact",
      artifact: "storyboard")                     → see what happened
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
