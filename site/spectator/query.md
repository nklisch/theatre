<script setup>
const messages0 = [
  { role: 'human', text: `The player is not picking up coins. Can you check if any coins are actually near the player?` },
  { role: 'agent', text: `Radius query returned 2 Area3D nodes within 3 meters: Coin_04 at 1.8m and Coin_07 at 2.6m. The coins are physically close. Let me check the collision configuration.` },
  { role: 'agent', text: `Found it. Coin_04 has collision_layer=4 and monitoring=true, but collision_mask=0. The Area3D is not monitoring any layers. The player is on layer 1, but the coin is not checking layer 1. Set the coin collision_mask to 1 (or whatever layer contains the player).` },
]
</script>

# spatial_query

Run geometric queries against the current game state.

`spatial_query` answers spatial relationship questions that would require math to answer from a snapshot. Instead of giving you all positions and asking you to compute distances, it computes them and returns sorted, filtered results.

## When to use it

- **"What is near X?"** — radius search
- **"What is closest to X?"** — nearest search
- **"What is in this area?"** — area/bounding-box search
- **"Does a ray from A hit B?"** — raycast
- **"How far is it to walk from A to B?"** — path distance
- **"What is the relationship between A and B?"** — relationship

## Query types

### `nearest`

Find the closest nodes to a point or node.

```json
{
  "query_type": "nearest",
  "from": "Player",
  "k": 5,
  "class_filter": ["CharacterBody3D", "Area3D"]
}
```

`from` can be a node name/path or a `[x, y, z]` coordinate.

**Response:**
```json
{
  "result": {
    "origin": [2.3, 0.0, -1.7],
    "results": [
      { "node": "Enemy_0", "class": "CharacterBody3D", "global_position": [-1.5, 0.0, 4.2], "distance": 6.4 },
      { "node": "Pickup_0", "class": "Area3D", "global_position": [3.0, 0.5, -1.0], "distance": 1.3 },
      { "node": "Enemy_1", "class": "CharacterBody3D", "global_position": [7.0, 0.0, -2.0], "distance": 4.7 }
    ]
  }
}
```

Results are sorted by `distance` ascending.

### `radius`

Find all nodes within a distance of a point.

```json
{
  "query_type": "radius",
  "from": "Player",
  "radius": 10.0,
  "class_filter": ["CollectibleItem", "Enemy"]
}
```

**Response:**
```json
{
  "result": {
    "origin": [2.3, 0.0, -1.7],
    "radius": 10.0,
    "results": [
      { "node": "Pickup_0", "class": "Area3D", "global_position": [3.0, 0.5, -1.0], "distance": 1.3 },
      { "node": "Enemy_1", "class": "CharacterBody3D", "global_position": [7.0, 0.0, -2.0], "distance": 4.7 }
    ]
  }
}
```

### `area`

Find nodes inside an axis-aligned bounding box.

```json
{
  "query_type": "area",
  "min": [-5.0, -1.0, -5.0],
  "max": [5.0, 3.0, 5.0]
}
```

**Response:**
```json
{
  "result": {
    "bounds": { "min": [-5, -1, -5], "max": [5, 3, 5] },
    "results": [
      { "node": "Player", "class": "CharacterBody3D", "global_position": [2.3, 0.0, -1.7] },
      { "node": "Enemy_0", "class": "CharacterBody3D", "global_position": [-3.1, 0.0, 4.2] }
    ]
  }
}
```

Note: `area` results do not include `distance` — there is no single reference point.

### `raycast`

Cast a ray from `from` in `direction` and report what it hits.

```json
{
  "query_type": "raycast",
  "from": "Player",
  "direction": [0.0, 0.0, -1.0],
  "max_distance": 20.0,
  "collision_mask": 3
}
```

**Response:**
```json
{
  "result": {
    "hit": true,
    "node": "Wall_North",
    "class": "StaticBody3D",
    "global_position": [2.3, 0.0, -8.0],
    "normal": [0.0, 0.0, 1.0],
    "distance": 6.3
  }
}
```

If no hit: `{ "result": { "hit": false } }`

The raycast is computed by querying the last known physics state — it uses the positions from the most recent collected frame, not a live physics query. For a live physics raycast, use `spatial_action` to call a method that performs `PhysicsDirectSpaceState3D.intersect_ray`.

### `path_distance`

Calculate the navigation mesh path distance between two points or nodes.

```json
{
  "query_type": "path_distance",
  "from": "Player",
  "to": "Enemy_0"
}
```

**Response:**
```json
{
  "result": {
    "from": [2.3, 0.0, -1.7],
    "to": [-3.1, 0.0, 4.2],
    "distance": 9.8,
    "reachable": true,
    "waypoints": [
      [2.3, 0.0, -1.7],
      [0.5, 0.0, 1.0],
      [-3.1, 0.0, 4.2]
    ]
  }
}
```

`reachable: false` means the navigation mesh has no path — the nodes are in disconnected regions. This is a very useful diagnostic for navmesh bugs.

### `relationship`

Describe the spatial relationship between two specific nodes.

```json
{
  "query_type": "relationship",
  "from": "Player",
  "to": "Enemy_0"
}
```

**Response:**
```json
{
  "result": {
    "from": "Player",
    "to": "Enemy_0",
    "distance": 6.4,
    "bearing_deg": 142.3,
    "relative": [-5.4, 0.0, 6.0],
    "occluded": false,
    "in_fov": true
  }
}
```

| Field | Description |
|---|---|
| `distance` | Euclidean distance in world units |
| `bearing_deg` | Horizontal angle from `from` to `to`, relative to `from`'s forward vector |
| `relative` | `[x, y, z]` offset from `from` to `to` in `from`'s local space |
| `occluded` | `true` if there is a `StaticBody3D` or `CSGShape3D` between them (line-of-sight check) |
| `in_fov` | `true` if `to` is within `from`'s frustum (for Camera3D `from`) |

## All parameters

| Parameter | Type | Applies to | Description |
|---|---|---|---|
| `query_type` | `string` | all | Query type: `nearest`, `radius`, `area`, `raycast`, `path_distance`, `relationship` |
| `from` | `string \| [x,y,z]` | all except `area` | Reference point, node, or start node |
| `to` | `string \| [x,y,z]` | `path_distance`, `relationship` | End node or point |
| `direction` | `[x,y,z]` | `raycast` | Normalized ray direction |
| `radius` | `float` | `radius` | Search radius in world units |
| `min` | `[x,y,z]` | `area` | Bounding box minimum corner |
| `max` | `[x,y,z]` | `area` | Bounding box maximum corner |
| `k` | `integer` | `nearest`, `radius` | Max results to return (default: 10) |
| `max_distance` | `float` | `raycast` | Max ray length (default: 100.0) |
| `collision_mask` | `integer` | `raycast` | Layer bitmask for ray collision |
| `class_filter` | `string[]` | `nearest`, `radius`, `area` | Filter to only these Godot classes |

## Example conversation

<AgentConversation :messages="messages0" />

## Tips

**Use `path_distance` to debug navmesh reachability.** If `reachable: false`, the navmesh is disconnected. If the distance is much longer than the straight-line distance, there may be unnecessary obstacles or the navmesh has a gap.

**Use `relationship` for line-of-sight bugs.** The `occluded` field tells you whether geometry blocks the path between two nodes — useful for AI perception debugging.

**`nearest` with `k: 1` is the fastest way to find "the closest thing."** Use it instead of sorting a radius result yourself.

**`area` is useful for zone-based queries.** "What nodes are in room B?" Define room B's bounding box and query the area.
