<script setup>
const messages0 = [
  { role: 'human', text: `The scene has 300 nodes and snapshots are huge. I only care about physics bodies and enemies.` },
  { role: 'agent', text: `Configuration updated. Spectator will now only track CharacterBody3D, RigidBody3D, Area3D, and Enemy nodes. Your snapshots should be much smaller now. Take a new snapshot to confirm.` },
  { role: 'human', text: `This is a slow turn-based game. I do not need 60Hz collection.` },
  { role: 'agent', text: `Collection rate reduced to 10 ticks per second. Memory usage and CPU overhead are reduced. Note: the ring buffer now covers 60 seconds of history at this rate (same 600-frame buffer).` },
]
</script>

# spatial_config

Configure Spectator's collection behavior.

`spatial_config` adjusts what Spectator tracks and how it tracks it. Changes take effect immediately and persist for the duration of the server session. They are not saved between sessions.

## When to use it

- **Reducing overhead**: lower the tick rate for performance-sensitive scenes
- **Expanding scope**: increase capture radius to track distant nodes
- **Narrowing scope**: track only specific node types to reduce noise
- **Adjusting buffer**: change ring buffer depth to record longer history

For most use cases, the defaults work well. You only need `spatial_config` when the default settings are insufficient for your specific investigation.

## Parameters

| Parameter | Type | Default | Description |
|---|---|---|---|
| `tick_rate` | `integer` | `60` | How many physics ticks to sample per second (1–120) |
| `capture_radius` | `float` | `200.0` | Maximum distance from scene origin to track nodes (meters) |
| `tracked_types` | `string[]` | see below | Godot class names to track |
| `buffer_depth_frames` | `integer` | `600` | Ring buffer size in frames (~10s at 60Hz) |
| `default_token_budget` | `integer` | `2000` | Default token budget for snapshot-style responses |
| `default_detail` | `string` | `"summary"` | Default detail level for snapshot responses |
| `record_path` | `string` | temp dir | Where to write clip files |

### Default tracked types

By default, Spectator tracks these Godot classes and all their subclasses:

- `CharacterBody3D`
- `RigidBody3D`
- `AnimatableBody3D`
- `Area3D`
- `Camera3D`
- `AnimationPlayer`
- `NavigationAgent3D`
- `NavigationObstacle3D`
- `Light3D`
- `GridMap`
- `TileMap`

UI nodes (`Control`, `CanvasLayer`, `Label`, etc.) are excluded by default. To track UI nodes, add them to `tracked_types`.

### `tick_rate`

The tick rate determines how many frames per second Spectator collects. The default of 60 matches Godot's default physics rate. Reducing the tick rate reduces CPU overhead and memory usage:

- `60` (default): Full fidelity, best for fast physics (projectiles, vehicles)
- `30`: Half fidelity, good for slower games (RPG, strategy)
- `10`: Low fidelity, good for turn-based or near-stationary debugging

Note: The tick rate cannot exceed Godot's physics rate. If your project runs at 30 physics ticks per second, setting `tick_rate: 60` has no effect.

### `capture_radius`

Nodes outside the `capture_radius` sphere (centered at `Vector3.ZERO`) are not tracked. Increase this for large open-world games; decrease it for small scenes to reduce noise.

```json
{
  "capture_radius": 500.0
}
```

For scenes where the player moves far from the origin (e.g., in a streaming open world), you may also want to set `capture_center` to the player's current position:

```json
{
  "capture_radius": 100.0,
  "capture_center": "Player"
}
```

When `capture_center` is a node name, the capture sphere follows that node.

### `tracked_types`

Override the list of tracked Godot classes. This replaces the default list:

```json
{
  "tracked_types": [
    "CharacterBody3D",
    "RigidBody3D",
    "Area3D",
    "EnemyBase"
  ]
}
```

To add a type without removing the defaults, use `extra_tracked_types`:

```json
{
  "extra_tracked_types": ["MyCustomNode", "BossEnemy"]
}
```

## Response format

`spatial_config` with no parameters returns the current configuration:

```json
{
  "tick_rate": 60,
  "capture_radius": 200.0,
  "capture_center": null,
  "buffer_depth_frames": 600,
  "buffer_depth_seconds": 10.0,
  "default_token_budget": 2000,
  "default_detail": "summary",
  "record_path": "/tmp/theatre-clips",
  "tracked_types": [
    "CharacterBody3D",
    "RigidBody3D",
    "Area3D",
    "Camera3D",
    "AnimationPlayer",
    "NavigationAgent3D"
  ],
  "extra_tracked_types": []
}
```

When you set parameters, the response echoes the new values:

```json
{
  "tick_rate": 30,
  "capture_radius": 200.0,
  "result": "ok"
}
```

## Example conversation

<AgentConversation :messages="messages0" />

## Tips

**Check current config before changing it.** Call `spatial_config` with no parameters to see what is active before you start adjusting values.

**Lower `tick_rate` for longer recordings.** At 60Hz, the ring buffer holds 10 seconds. At 10Hz, the same buffer holds 60 seconds without using a clip file.

**Use `capture_center: "Player"`** in large open worlds. This ensures you always track nodes near the player, even as they move hundreds of meters from the origin.

**Restore defaults after targeted investigations.** If you narrowed `tracked_types` for a specific debug session, reset it before starting a new investigation: `spatial_config { "tracked_types": null }` restores defaults.
