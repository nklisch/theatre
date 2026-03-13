<script setup>
const messages0 = [
  { role: 'human', text: `I added a JumpPad Area3D to the level. Wire it up to launch the player when they step on it.` },
  { role: 'agent', text: `I need to connect the JumpPad body_entered signal to a handler on the JumpPad node (which should have a script with _on_body_entered). Let me check what signals are available.` },
  { role: 'agent', text: `JumpPad is an Area3D with monitoring=true, collision_mask=1 (player layer). No signals connected yet. Connecting body_entered to the JumpPad script.` },
  { role: 'agent', text: `Signal connected. When the player steps on JumpPad, _on_body_entered will be called on the JumpPad script. Make sure that script has a _on_body_entered method that applies the upward velocity to the body.` },
  { role: 'human', text: `Also set the launch_force export to 20 on the JumpPad.` },
  { role: 'agent', text: `Done. launch_force = 20.0 set on JumpPad.` },
]
</script>

# Scene Wiring

Connect signals, set exported variables, and manage node metadata.

"Wiring" refers to the relationships between nodes that are not part of the scene tree hierarchy — signal connections, export variable values, and metadata entries.

## Operations

### `signal_connect`

Connect a signal from one node to a method on another.

```json
{
  "op": "signal_connect",
  "project_path": "/home/user/my-game",
  "scene": "scenes/level_01.tscn",
  "from_node": "Level/Enemy_0/DetectionZone",
  "signal": "body_entered",
  "to_node": "Level/Enemy_0",
  "method": "_on_detection_zone_body_entered"
}
```

| Parameter | Type | Description |
|---|---|---|
| `from_node` | `string` | Node that owns the signal |
| `signal` | `string` | Signal name |
| `to_node` | `string` | Node that owns the receiving method |
| `method` | `string` | Method name to call when signal fires |
| `flags` | `integer` | Connection flags (default 0; use 1 for one-shot) |
| `binds` | `array` | Additional arguments to pass when signal fires |

**Response:**
```json
{
  "op": "signal_connect",
  "from_node": "Level/Enemy_0/DetectionZone",
  "signal": "body_entered",
  "to_node": "Level/Enemy_0",
  "method": "_on_detection_zone_body_entered",
  "result": "ok"
}
```

### `signal_disconnect`

Remove a signal connection.

```json
{
  "op": "signal_disconnect",
  "project_path": "/home/user/my-game",
  "scene": "scenes/level_01.tscn",
  "from_node": "Level/Enemy_0/DetectionZone",
  "signal": "body_entered",
  "to_node": "Level/Enemy_0",
  "method": "_on_detection_zone_body_entered"
}
```

**Response:**
```json
{
  "op": "signal_disconnect",
  "from_node": "Level/Enemy_0/DetectionZone",
  "signal": "body_entered",
  "result": "ok"
}
```

### `signal_list`

List all signal connections on a node.

```json
{
  "op": "signal_list",
  "project_path": "/home/user/my-game",
  "scene": "scenes/level_01.tscn",
  "node": "Level/Enemy_0/DetectionZone"
}
```

**Response:**
```json
{
  "signals": [
    {
      "signal": "body_entered",
      "to_node": "Level/Enemy_0",
      "method": "_on_detection_zone_body_entered",
      "flags": 0
    },
    {
      "signal": "body_exited",
      "to_node": "Level/Enemy_0",
      "method": "_on_detection_zone_body_exited",
      "flags": 0
    }
  ]
}
```

### `export_set`

Set the value of an `@export` variable on a node's script.

```json
{
  "op": "export_set",
  "project_path": "/home/user/my-game",
  "scene": "scenes/enemy.tscn",
  "node": "Enemy",
  "property": "patrol_speed",
  "value": 3.5
}
```

This is functionally identical to `node_set_property` — both set properties via Godot's property system. The distinction is conceptual: `export_set` is for script-defined `@export` variables, while `node_set_property` is for built-in Godot node properties.

**Response:**
```json
{
  "op": "export_set",
  "node": "Enemy",
  "property": "patrol_speed",
  "value": 3.5,
  "result": "ok"
}
```

### Setting multiple exports at once

```json
{
  "op": "export_set",
  "project_path": "/home/user/my-game",
  "scene": "scenes/enemy.tscn",
  "node": "Enemy",
  "properties": {
    "patrol_speed": 3.5,
    "alert_speed": 8.0,
    "detection_range": 10.0,
    "attack_damage": 25,
    "health": 100
  }
}
```

## Example conversation: Wiring a jump pad

<AgentConversation :messages="messages0" />

## Common signal connections

### Area3D detection
```json
{
  "from_node": "Enemy/DetectionZone",
  "signal": "body_entered",
  "to_node": "Enemy",
  "method": "_on_body_entered"
}
```

### Button press
```json
{
  "from_node": "UI/HUD/AttackButton",
  "signal": "pressed",
  "to_node": "Player",
  "method": "_on_attack_button_pressed"
}
```

### Timer timeout
```json
{
  "from_node": "Enemy/AttackCooldownTimer",
  "signal": "timeout",
  "to_node": "Enemy",
  "method": "_on_attack_cooldown_timeout"
}
```

### AnimationPlayer finished
```json
{
  "from_node": "Player/AnimationPlayer",
  "signal": "animation_finished",
  "to_node": "Player",
  "method": "_on_animation_finished"
}
```

## Tips

**Check existing connections before adding new ones.** Use `signal_list` to avoid creating duplicate connections — Godot will error or double-fire if the same connection is added twice.

**The `to_node` method must exist.** Director validates that the `to_node` has a script, but not that the method exists in that script. A missing method will cause a runtime error when the signal fires. Double-check method names.

**Use `spatial_inspect` with `include: ["signals"]`** to see connections in the running game. This is the fastest way to verify that wiring applied correctly.

**`export_set` is the right tool for game designer parameters.** Things like enemy health, damage, speed, and range are usually `@export` variables — use `export_set` to tune them without touching the script.
