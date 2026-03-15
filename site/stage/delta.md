<script setup>
import { data } from '../.vitepress/data/tools.data'
const params = data.params['spatial_delta'] ?? []

const messages0 = [
  { role: 'human', text: `I started the game. The player is at the spawn point. Watch for changes as I start moving.` },
  { role: 'agent', text: `Got it. Let me take a baseline snapshot first.` },
  { role: 'agent', text: `Baseline captured. Player at [0.0, 0.0, 0.0], all enemies stationary. Move around and I will check the delta.` },
  { role: 'human', text: `I walked forward and jumped. Check the delta.` },
  { role: 'agent', text: `Delta since baseline: Player moved from [0.0, 0.0, 0.0] to [3.2, 0.0, -4.1], velocity is now [0.0, -3.8, -2.0] (falling). Enemy_2 also moved slightly — it started patrolling toward the player at velocity [0.8, 0.0, 0.0]. Everything else is unchanged.` },
]
</script>

# spatial_delta

Get only what changed since the last baseline snapshot.

`spatial_delta` is the efficient alternative to repeated `spatial_snapshot` calls. Instead of returning all tracked nodes, it returns only nodes whose tracked properties changed since the stored baseline. In a scene where most nodes are stationary, this can be 10-50x smaller than a full snapshot.

## When to use it

- **Polling for changes**: "What moved since my last check?"
- **After a game event**: "What changed after the enemy spawned?"
- **Watch polling**: Reading accumulated changes since the baseline
- **Verifying a fix**: "Did the teleport land the player where I expected?"

Do **not** use `spatial_delta` as your first call in a session — use `spatial_snapshot` first to get oriented and establish the baseline. Delta computes changes relative to that stored baseline.

## How the baseline works

When you call `spatial_snapshot`, the server stores the response as the **baseline**. Every subsequent `spatial_delta` call computes what changed since that baseline. There is no `since_frame` parameter — the baseline is set automatically by the most recent snapshot.

To update the baseline, call `spatial_snapshot` again. This is the normal pattern for watch loops:

```
spatial_snapshot   → baseline established
... game runs ...
spatial_delta      → what changed since snapshot?
spatial_snapshot   → new baseline established
... game runs ...
spatial_delta      → what changed since second snapshot?
```

## Parameters

<ParamTable :params="params" />

## Response format

```json
{
  "frame": 284,
  "baseline_frame": 120,
  "elapsed_ms": 2733,
  "changed_node_count": 2,
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
| `frame` | The current frame when the delta was computed |
| `baseline_frame` | The frame from the most recent `spatial_snapshot` |
| `elapsed_ms` | Milliseconds between baseline_frame and frame |
| `changed_node_count` | Number of nodes with changes |
| `nodes` | Map of node name → changed properties only |

**Only changed properties are included** in each node entry. If the player's position changed but velocity did not, only `global_position` appears in the player's entry.

## Example conversation

<AgentConversation :messages="messages0" />

## Using delta in a watch loop

The typical watch pattern is:

1. Call `spatial_snapshot` to get the current state and establish the baseline
2. Optionally call `spatial_watch` on nodes of interest
3. Periodically call `spatial_delta` to check changes since the baseline
4. Call `spatial_snapshot` again when you want a new baseline

```
spatial_snapshot   → baseline
... game runs ...
spatial_delta      → player moved, enemy_0 moved
spatial_snapshot   → new baseline
... game runs ...
spatial_delta      → enemy_0 changed velocity
```

Each delta response is small because it only includes actual changes.

## Tips

**Start with `spatial_snapshot`, then use deltas.** Snapshot establishes the baseline that delta compares against. Without a prior snapshot, there is no baseline.

**Call `spatial_snapshot` to reset the baseline.** If you want to start tracking from a fresh state (for example, after making a change with `spatial_action`), call `spatial_snapshot` again to update the baseline.

**Use `class_filter` to focus on relevant nodes.** If you are debugging enemies, filter to `CharacterBody3D` to avoid including unrelated node changes in your delta.

**Delta responses include only changed properties.** If the player's position changed but rotation didn't, you only see `global_position` in the response. This is intentional — it keeps responses small and makes changes obvious.
