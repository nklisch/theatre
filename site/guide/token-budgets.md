# Token Budgets

Large Godot scenes can have hundreds of nodes. A fully detailed snapshot of a 200-node scene would easily exceed 50,000 tokens — blowing up your context window and making the response useless. Theatre manages this problem through token budgets and detail levels.

## The problem

When you call `spatial_snapshot`, the server has to decide how much data to include. More data means:
- More tokens consumed from your context window
- Slower response time
- More noise for the agent to filter through
- Higher cost per tool call

Less data means the agent might miss relevant nodes.

Theatre solves this with a **budget-first design**: every data-returning tool accepts a `budget_tokens` parameter and a `detail` level. The server measures the response as it builds it and stops adding data once the budget is reached. The most relevant data is always included first.

## Detail levels

The `detail` parameter controls how much information is included per node.

### `summary` (default)

Returns only the essential spatial information for each node:

```json
{
  "Player": {
    "class": "CharacterBody3D",
    "global_position": [2.3, 0.0, -1.7],
    "velocity": [0.0, -2.4, 0.0]
  }
}
```

This is roughly 80-120 tokens per node. Good for answering "where is everything?" questions when you do not need property details.

### `full`

Returns all tracked properties:

```json
{
  "Player": {
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
}
```

This is roughly 300-500 tokens per node. Good when you need the complete picture for a specific set of nodes.

### `custom`

Include only the properties you specify:

```json
{
  "detail": "custom",
  "include_properties": ["global_position", "velocity", "collision_layer"]
}
```

Use `custom` when you know exactly what you need and want to minimize tokens.

## The `budget_tokens` parameter

Every snapshot-style tool accepts `budget_tokens` (integer, default 2000):

```json
{
  "detail": "summary",
  "budget_tokens": 500
}
```

The server builds the response node by node, measuring token usage as it goes. When adding the next node would exceed `budget_tokens`, it stops and includes a `truncated: true` flag in the response:

```json
{
  "frame": 412,
  "node_count": 200,
  "included_nodes": 12,
  "truncated": true,
  "summary": { ... 12 nodes ... }
}
```

The `node_count` tells the agent how many nodes exist; `included_nodes` tells it how many were included. If the response is truncated, the agent knows to narrow its query.

## Priority ordering

When the budget forces truncation, which nodes are included first?

**If `focus_node` is set**: The focus node is always included first, then nodes are included in ascending order of distance from the focus node.

**If `include_types` is set**: Nodes matching those types are included first, then others.

**Otherwise**: Nodes are included in scene tree order (breadth-first from root).

This means `budget_tokens: 500` with `focus_node: "Player"` gives you the player plus the nearest nodes, not the first 5 nodes in the tree (which are often UI elements or root containers).

## Practical budget guidelines

| Scenario | Recommended settings |
|---|---|
| "Where is everything?" overview | `detail: summary`, `budget_tokens: 2000` |
| Focus on one area of the scene | `detail: summary`, `focus_node: "Player"`, `budget_tokens: 1000` |
| Debug a specific node | Use `spatial_inspect` instead of `snapshot` |
| Monitor changes over time | Use `spatial_delta` — only changed nodes included |
| Check scene structure (no spatial data) | Use `scene_tree` — very compact |
| Small scene (< 20 nodes) | `detail: full`, `budget_tokens: 5000` |

## `spatial_delta` as the budget-friendly choice

When the game is running and you want to know what changed since the last snapshot, use `spatial_delta` instead of `spatial_snapshot`. Delta responses only include nodes whose tracked properties changed since the given frame:

```json
{
  "since_frame": 400,
  "changed_nodes": {
    "Player": {
      "global_position": [3.1, 0.0, -2.3],
      "velocity": [2.0, 0.0, 0.0]
    }
  }
}
```

If only the player moved, you get only the player — not 200 unchanged nodes. This is often 10-50x smaller than a full snapshot.

## `spatial_inspect` for single nodes

When you need full detail on exactly one node, `spatial_inspect` is more efficient than a full snapshot:

```json
{
  "node": "EnemyDetectionZone",
  "include": ["properties", "spatial_context"]
}
```

This returns every tracked property of the node plus its spatial context (nearby nodes, parent/child relationships). No budget truncation needed — a single node's data is always manageable.

## Recording budget behavior

The `recording` tool's `query_range` action has its own budget parameter (`max_frames`). When querying a large frame range, use:

```json
{
  "action": "query_range",
  "clip_id": "clip_01",
  "start_frame": 300,
  "end_frame": 325,
  "nodes": ["Player", "Enemy"],
  "detail": "summary"
}
```

Specifying `nodes` limits data to only those nodes across all frames. `detail: summary` keeps per-frame data compact.

## Adjusting the default budget

If you consistently want a different default budget, set `budget_tokens` in `spatial_config`:

```json
{
  "default_budget_tokens": 3000,
  "default_detail": "summary"
}
```

This persists for the duration of the server session.
