<script setup>
const messages0 = [
  { role: 'human', text: `I started the game. The player is at the spawn point. Watch for changes as I start moving.` },
  { role: 'agent', text: `Got it. Let me take a baseline snapshot first.` },
  { role: 'agent', text: `Baseline captured at frame 120. Player at [0.0, 0.0, 0.0], all enemies stationary. Move around and I will check the delta.` },
  { role: 'human', text: `I walked forward and jumped. Check the delta.` },
  { role: 'agent', text: `Delta from frame 120 to frame 284 (2.7 seconds): Player moved from [0.0, 0.0, 0.0] to [3.2, 0.0, -4.1], velocity is now [0.0, -3.8, -2.0] (falling). Enemy_2 also moved slightly — it started patrolling toward the player at velocity [0.8, 0.0, 0.0]. Everything else is unchanged.` },
]
</script>

# spatial_delta

Get only what changed since a specific frame.

`spatial_delta` is the efficient alternative to repeated `spatial_snapshot` calls. Instead of returning all tracked nodes, it returns only nodes whose tracked properties changed since the given frame. In a scene where most nodes are stationary, this can be 10-50x smaller than a full snapshot.

## When to use it

- **Polling for changes**: "What moved in the last 2 seconds?"
- **After a game event**: "What changed after the enemy spawned?"
- **Watch polling**: Reading accumulated changes since your last check
- **Debugging a transition**: "What happened between frame 300 and frame 340?"

Do **not** use `spatial_delta` as your first call in a session — use `spatial_snapshot` first to get oriented. You need a reference frame number to compute a useful delta.

## Parameters

| Parameter | Type | Default | Description |
|---|---|---|---|
| `since_frame` | `integer` | required | The reference frame. Returns changes since this frame. |
| `token_budget` | `integer` | `1000` | Token budget for the response |
| `class_filter` | `string[]` | `null` | Only include changes to nodes of these classes |
| `min_distance_change` | `float` | `0.01` | Minimum position change (meters) to count as a change |
| `min_velocity_change` | `float` | `0.1` | Minimum velocity change to count as a change |

### `since_frame`

The frame number from a previous `spatial_snapshot`, `spatial_delta`, or `clips` response. Everything that changed after this frame is included.

If `since_frame` is older than the ring buffer depth (default: 600 frames = 10 seconds), the server returns an error. Use a recent frame or start a clip recording if you need longer history.

### `min_distance_change`

By default, position changes smaller than 0.01 meters are ignored as noise (floating-point jitter, micro-corrections). Increase this threshold to only report significant movement:

```json
{
  "since_frame": 400,
  "min_distance_change": 0.5
}
```

This only reports nodes that moved more than 0.5 meters since frame 400 — useful for tracking large movements during an animation or cutscene.

## Response format

```json
{
  "from_frame": 400,
  "to_frame": 450,
  "elapsed_ms": 833,
  "changed_node_count": 2,
  "unchanged_node_count": 10,
  "nodes": {
    "Player": {
      "class": "CharacterBody3D",
      "global_position": [3.1, 0.0, -2.3],
      "velocity": [2.0, 0.0, 0.0],
      "on_floor": true
    },
    "Enemy_0": {
      "class": "CharacterBody3D",
      "global_position": [-1.5, 0.0, 4.2],
      "velocity": [1.2, 0.0, 0.5]
    }
  }
}
```

| Field | Description |
|---|---|
| `from_frame` | The `since_frame` you requested |
| `to_frame` | The current frame when the delta was computed |
| `elapsed_ms` | Milliseconds between from_frame and to_frame |
| `changed_node_count` | Number of nodes with changes |
| `unchanged_node_count` | Number of nodes that did not change |
| `nodes` | Map of node name → changed properties only |

**Only changed properties are included** in each node entry. If the player's position changed but velocity did not, only `global_position` appears in the player's entry.

## Example conversation

<AgentConversation :messages="messages0" />

## Using delta in a watch loop

The typical watch pattern is:

1. Call `spatial_snapshot` to get the current frame number
2. Call `spatial_watch` on nodes of interest
3. Periodically call `spatial_delta { "since_frame": last_frame }` to check changes
4. Update `last_frame` to the `to_frame` from each delta response

```
Frame 100: snapshot (baseline)
... game runs ...
Frame 160: delta since 100 → player moved
Frame 220: delta since 160 → player and enemy_0 moved
Frame 280: delta since 220 → enemy_0 changed velocity
```

Each delta response is small because it only includes actual changes.

## Tips

**Start with `spatial_snapshot`, then use deltas.** You need a frame number to compute a delta. The snapshot gives you one.

**Keep `since_frame` recent.** The ring buffer holds ~10 seconds of frames by default. If your `since_frame` is older, you'll get an error. If you need longer history, use the `clips` tool to record to disk.

**Use `min_distance_change` to filter noise.** Physics simulation produces tiny floating-point perturbations even on "stationary" objects. The default 0.01m threshold handles most cases, but increase it if you're seeing a lot of stationary-looking objects in your delta.

**Delta responses include only changed properties.** If the player's position changed but rotation didn't, you only see `global_position` in the response. This is intentional — it keeps responses small and makes changes obvious.
