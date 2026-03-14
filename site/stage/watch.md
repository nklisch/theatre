<script setup>
import { data } from '../.vitepress/data/tools.data'
const params = data.params['spatial_watch'] ?? []

const messages0 = [
  { role: 'human', text: `I want to monitor the player health and position during this boss fight. Set that up.` },
  { role: 'agent', text: `Watch created (watch_id: w_a1b2c3). I am now tracking Player.position, .velocity, and .health. Do the boss fight and tell me when something goes wrong — I will check the delta.` },
  { role: 'human', text: `I just took a big hit from the boss and my health looks wrong. Check it.` },
  { role: 'agent', text: `Delta from frame 100 to frame 487 (6.5 seconds): Player health dropped from 100 to 15 across 3 hit events. The final hit at frame 461 reduced health by 40 — that was the large hit. Position moved significantly (traveled about 12 meters). The health drain rate looks correct for the boss damage values, but frame 461 shows an unusually large single hit. Let me check what happened at that frame.` },
]
</script>

# spatial_watch

Monitor nodes continuously for changes.

`spatial_watch` registers one or more nodes for continuous tracking. Once a watch is set, the Stage server polls those nodes every tick and accumulates changes. You then read the changes by calling `spatial_delta` — only changed values since your last poll are returned.

Watches are useful when you want to observe a node over a long session without being surprised by what changed between tool calls.

## When to use it

- **Monitoring player stats**: health, position, velocity over a gameplay session
- **Watching an enemy's state machine**: current state, target, velocity
- **Tracking animation state**: which animation is playing, at what position
- **Debugging a slow bug**: something happens every 30-60 seconds — watch and wait
- **Automated QA**: set watches on all critical nodes, run through the level, check deltas

Do **not** set a watch on every node in your scene — watches add tracking overhead. Use watches for the nodes you specifically want to follow.

## Parameters

<ParamTable :params="params" />

### Other actions

| Action | Parameters | Description |
|---|---|---|
| `"list"` | — | List all active watches |
| `"delete"` | `watch_id: string` | Remove a watch |
| `"clear"` | — | Remove all watches |

## Trackable properties

The `track` array specifies which properties to monitor. Common values:

| Track value | Godot property | Node types |
|---|---|---|
| `"position"` | `global_position` | All spatial nodes |
| `"velocity"` | `velocity` / `linear_velocity` | Physics bodies |
| `"rotation"` | `rotation_deg` | All spatial nodes |
| `"health"` | User-defined export | Any node with `@export var health` |
| `"animation"` | `current_animation` | AnimationPlayer |
| `"animation_position"` | `current_animation_position` | AnimationPlayer |
| `"visible"` | `visible` | All nodes |
| `"collision_layer"` | `collision_layer` | Physics bodies, Area3D |
| `"target_position"` | `target_position` | NavigationAgent3D |
| `"is_navigation_finished"` | `is_navigation_finished` | NavigationAgent3D |

For user-defined properties (like `health`), use the exported property name exactly as defined in GDScript.

## Response format

### Create response

```json
{
  "watch_id": "w_a1b2c3",
  "node": "Player",
  "track": ["position", "velocity", "health"],
  "active_watches": 2
}
```

The `watch_id` is used to delete or reference this specific watch. It is stable for the duration of the server session.

### List response

```json
{
  "watches": [
    {
      "watch_id": "w_a1b2c3",
      "node": "Player",
      "track": ["position", "velocity", "health"],
      "created_frame": 100
    },
    {
      "watch_id": "w_d4e5f6",
      "node": "Enemy_0",
      "track": ["position", "velocity"],
      "created_frame": 150
    }
  ]
}
```

### Delete response

```json
{
  "watch_id": "w_a1b2c3",
  "result": "ok"
}
```

## Example conversation

<AgentConversation :messages="messages0" />

## Watching multiple nodes

You can create multiple watches — one per `spatial_watch` call:

```json
{ "action": "create", "node": "Player", "track": ["position", "health"] }
{ "action": "create", "node": "Boss", "track": ["position", "velocity", "phase"] }
{ "action": "create", "node": "Boss/HealthBar", "track": ["health"] }
```

All watched nodes appear in subsequent `spatial_delta` responses whenever their tracked properties change.

## Polling interval

Watches accumulate changes continuously in the background. Your `spatial_delta` calls can be as infrequent or frequent as needed — all changes since the last poll are batched into one response. You do not need to poll on a schedule; just call `spatial_delta` when you want to see what happened.

## Tips

**Keep watch lists focused.** Watching 10 properties on 5 nodes is fine. Watching 20 nodes with full property lists is wasteful — use `spatial_snapshot` for that.

**Save the `watch_id`.** The agent should remember the `watch_id` returned when the watch was created. You will need it to delete the watch when done.

**Clear watches between investigations.** If you switch from debugging one feature to another, call `spatial_watch { "action": "clear" }` to reset. Stale watches from a previous investigation add noise to your deltas.

**Combine with recording for post-mortem analysis.** Watches show what is changing live; recordings capture everything for later analysis. During complex debug sessions, run both simultaneously.
