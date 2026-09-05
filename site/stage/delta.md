---
description: "Spatial Delta tracks what changed between snapshots — moved nodes, new nodes, removed nodes, and property changes."
---

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

`spatial_delta` is the focused alternative to repeated `spatial_snapshot` calls. Instead of returning all tracked nodes, it returns only supported changes since the stored baseline.

## When to use it

- **Polling for changes**: "What moved since my last check?"
- **After a game event**: "What changed after the enemy spawned?"
- **Watch polling**: Reading accumulated changes since the baseline
- **Verifying a fix**: "Did the teleport land the player where I expected?"

Do **not** use `spatial_delta` as your first call in a session — use `spatial_snapshot` first to get oriented and establish the baseline. Delta computes changes relative to that stored baseline.

Use this workflow in one persistent MCP session. Separate CLI snapshot and delta calls do not share a baseline; the CLI rejects delta requests with `persistent_session_required`.

## How the baseline works

A `spatial_snapshot` establishes the first baseline. Each `spatial_delta` compares
the current observation with the stored baseline and then advances that baseline
to the current frame. An action with `return_delta: true` does the same when a
baseline exists. There is no caller-supplied `since_frame` parameter.

```
spatial_snapshot   → baseline established
... game runs ...
spatial_delta      → changes since snapshot; baseline advances
... game runs ...
spatial_delta      → changes since the prior delta; baseline advances
```

## Parameters

<ParamTable :params="params" />

## Response semantics

The response identifies `from_frame` and `to_frame`, then includes only non-empty
change categories such as movement, state changes, entered or exited entities,
signals, and watch triggers. See the [generated Stage reference](/api/) for the
current structural schema.

## Example conversation

<AgentConversation :messages="messages0" />

## Using delta in a watch loop

The typical watch pattern is:

1. Call `spatial_snapshot` to get the current state and establish the baseline
2. Optionally call `spatial_watch` on nodes of interest
3. Call `spatial_delta` when you want changes since the prior baseline or delta.
4. Call `spatial_snapshot` again when you need a fresh full view and replacement baseline.

Each delta response advances the baseline and omits empty change categories.

## Tips

**Start with `spatial_snapshot`, then use deltas.** Snapshot establishes the baseline that delta compares against. Without a prior snapshot, there is no baseline.

**Call `spatial_snapshot` to reset the baseline.** If you want to start tracking from a fresh state (for example, after making a change with `spatial_action`), call `spatial_snapshot` again to update the baseline.

**Use `class_filter` to focus on relevant nodes.** If you are debugging enemies, filter to `CharacterBody3D` to avoid including unrelated node changes in your delta.

**Delta responses include only changed properties.** If the player's position changed but rotation didn't, you only see `global_position` in the response. This is intentional — it keeps responses small and makes changes obvious.
