<script setup>
const messages0 = [
  { role: 'human', text: `The game is running. What does my scene look like right now?` },
  { role: 'agent', text: `Let me take a spatial snapshot to see the current state.` },
  { role: 'agent', text: `I can see 8 nodes in your scene. The Player (CharacterBody3D) is at [2.3, 0.0, -1.7] with zero velocity — standing still. There are 3 enemies: Enemy_0 at [-3.1, 0.0, 4.2] moving at 0.8 units/sec along X, Enemy_1 at [5.0, 0.0, 1.1] stationary, Enemy_2 at [0.0, 0.0, -8.3] stationary. The Camera3D follows the player at [2.3, 1.8, 0.3]. There are also 3 StaticBody3D platforms and a SpotLight3D.` },
  { role: 'human', text: `Focus on the player and anything nearby.` },
  { role: 'agent', text: `Player details: on_floor=true, collision_layer=1, collision_mask=3, velocity=[0,0,0]. Enemy_0 is the closest at 5.7 units — also on layer 1. Enemy_1 is 2.9 units to the right. Both enemies are in motion toward the player.` },
]
</script>

# spatial_snapshot

Get an instant picture of every tracked node in the running game.

`spatial_snapshot` is the entry point for most Spectator sessions. It returns positions, velocities, and key properties for all nodes the collector is tracking, subject to your token budget.

## When to use it

- **Starting a new investigation**: "What is the current state of my scene?"
- **After a major game event**: "What are the positions now that the enemy has spawned?"
- **Confirming a fix**: "Does the player spawn at the right position now?"
- **Orientation**: Before using more targeted tools, snapshot gives context.

Do **not** use `spatial_snapshot` repeatedly to detect changes — use `spatial_delta` instead. Snapshots include all tracked nodes even if nothing changed; deltas only include what changed.

## Parameters

| Parameter | Type | Default | Description |
|---|---|---|---|
| `detail` | `"summary" \| "standard" \| "full"` | `"summary"` | How much data to include per node |
| `token_budget` | `integer` | `2000` | Approximate token budget for the response |
| `focal_node` | `string` | `null` | Scene path of a node to prioritize in the response |
| `class_filter` | `string[]` | `null` | Only include nodes of these Godot classes |
| `include_properties` | `string[]` | `null` | Additional properties to include alongside standard fields |

### `detail` values

**`"summary"`** — Position and velocity only (approximately 80-120 tokens per node):
```json
{
  "class": "CharacterBody3D",
  "global_position": [2.3, 0.0, -1.7],
  "velocity": [0.0, -2.4, 0.0]
}
```

**`"full"`** — All tracked properties (approximately 300-500 tokens per node):
```json
{
  "class": "CharacterBody3D",
  "global_position": [2.3, 0.0, -1.7],
  "rotation_deg": [0.0, 45.2, 0.0],
  "velocity": [0.0, -2.4, 0.0],
  "scale": [1.0, 1.0, 1.0],
  "visible": true,
  "collision_layer": 1,
  "collision_mask": 3,
  "on_floor": false,
  "on_wall": false
}
```

**`"standard"`** — Position, velocity, rotation, scale, and common flags (approximately 150-250 tokens per node).

### `focal_node`

When set, the focal node is always included in the response (even if it would be cut by the budget), and nearby nodes are prioritized over distant nodes. Useful when debugging a specific character or object in a large scene.

```json
{
  "detail": "summary",
  "focal_node": "World/Player",
  "token_budget": 1000
}
```

### Filtering by type

```json
{
  "detail": "full",
  "class_filter": ["CharacterBody3D", "RigidBody3D"],
  "token_budget": 3000
}
```

This returns only physics bodies, ignoring cameras, lights, UI nodes, etc.

## Response format

```json
{
  "frame": 412,
  "timestamp_ms": 6867,
  "node_count": 47,
  "included_nodes": 12,
  "truncated": false,
  "nodes": {
    "Player": {
      "class": "CharacterBody3D",
      "path": "World/Player",
      "global_position": [2.3, 0.0, -1.7],
      "velocity": [0.0, 0.0, 0.0]
    },
    "Enemy_0": {
      "class": "CharacterBody3D",
      "path": "World/Enemies/Enemy_0",
      "global_position": [-3.1, 0.0, 4.2],
      "velocity": [0.8, 0.0, 0.0]
    },
    "Camera3D": {
      "class": "Camera3D",
      "path": "World/Player/Camera3D",
      "global_position": [2.3, 1.8, 0.3]
    }
  }
}
```

| Field | Description |
|---|---|
| `frame` | Physics frame number when data was collected |
| `timestamp_ms` | Milliseconds since game start |
| `node_count` | Total tracked nodes in the scene |
| `included_nodes` | Nodes included in this response |
| `truncated` | `true` if budget was reached before including all nodes |
| `nodes` | Map of node name → node data |

### Node data fields

| Field | Description |
|---|---|
| `class` | Godot class name |
| `path` | Full scene tree path from root |
| `global_position` | `[x, y, z]` in world space |
| `velocity` | `[x, y, z]` units/second (for physics bodies) |
| `rotation_deg` | `[x, y, z]` rotation in degrees (for `full` detail) |
| `scale` | `[x, y, z]` scale (for `full` detail) |
| `visible` | Boolean visibility (for `full` detail) |
| `collision_layer` | Bitmask integer (for physics nodes, `full` detail) |
| `collision_mask` | Bitmask integer (for physics nodes, `full` detail) |
| `on_floor` | `true` if CharacterBody3D is on a floor surface |
| `on_wall` | `true` if CharacterBody3D is on a wall surface |

## Example conversation

<AgentConversation :messages="messages0" />

## Tips

**Use `focal_node` in large scenes.** Without it, the budget may be exhausted by nodes you do not care about (terrain, lights, UI containers). Setting `focal_node: "Player"` ensures the player and nearby objects are always in the response.

**Combine with `class_filter` for targeted investigations.** If you are debugging pathfinding, filter to `NavigationAgent3D` and `CharacterBody3D`. If you are debugging physics, filter to physics body types.

**One snapshot per investigation start.** Do not call snapshot in a loop. Call it once, orient the agent, then use `spatial_delta`, `spatial_inspect`, or `spatial_query` for subsequent questions.

**Check `truncated`.** If `truncated: true`, the budget was hit. Either increase `token_budget`, add a `class_filter` to reduce scope, or use `spatial_query` to search a smaller area.
