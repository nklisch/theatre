<script setup>
import { data } from '../.vitepress/data/tools.data'

const signal_connect = data.params['signal_connect'] ?? []
const signal_disconnect = data.params['signal_disconnect'] ?? []
const signal_list = data.params['signal_list'] ?? []

const messages0 = [
  { role: 'human', text: `I added a JumpPad Area3D to the level. Wire it up to launch the player when they step on it.` },
  { role: 'agent', text: `I need to connect the JumpPad body_entered signal to a handler on the JumpPad node (which should have a script with _on_body_entered). Let me check what signals are available.` },
  { role: 'agent', text: `JumpPad is an Area3D with monitoring=true, collision_mask=1 (player layer). No signals connected yet. Connecting body_entered to the JumpPad script.` },
  { role: 'agent', text: `Signal connected. When the player steps on JumpPad, _on_body_entered will be called on the JumpPad script. Make sure that script has a _on_body_entered method that applies the upward velocity to the body.` },
  { role: 'human', text: `Also set the launch_force property to 20 on the JumpPad.` },
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
  "scene_path": "scenes/level_01.tscn",
  "source_path": "Level/Enemy_0/DetectionZone",
  "signal_name": "body_entered",
  "target_path": "Level/Enemy_0",
  "method_name": "_on_detection_zone_body_entered"
}
```

<ParamTable :params="signal_connect" />

**Response:**
```json
{
  "op": "signal_connect",
  "source_path": "Level/Enemy_0/DetectionZone",
  "signal_name": "body_entered",
  "target_path": "Level/Enemy_0",
  "method_name": "_on_detection_zone_body_entered",
  "result": "ok"
}
```

### `signal_disconnect`

Remove a signal connection.

```json
{
  "op": "signal_disconnect",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/level_01.tscn",
  "source_path": "Level/Enemy_0/DetectionZone",
  "signal_name": "body_entered",
  "target_path": "Level/Enemy_0",
  "method_name": "_on_detection_zone_body_entered"
}
```

<ParamTable :params="signal_disconnect" />

**Response:**
```json
{
  "op": "signal_disconnect",
  "source_path": "Level/Enemy_0/DetectionZone",
  "signal_name": "body_entered",
  "result": "ok"
}
```

### `signal_list`

List all signal connections in a scene, optionally filtered to a specific node.

```json
{
  "op": "signal_list",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/level_01.tscn",
  "node_path": "Level/Enemy_0/DetectionZone"
}
```

Omit `node_path` to list signals for all nodes in the scene.

<ParamTable :params="signal_list" />

**Response:**
```json
{
  "signals": [
    {
      "signal_name": "body_entered",
      "source_path": "Level/Enemy_0/DetectionZone",
      "target_path": "Level/Enemy_0",
      "method_name": "_on_detection_zone_body_entered",
      "flags": 0
    }
  ]
}
```

## Setting `@export` variables

`@export` variables are set the same way as any built-in property — use `node_set_properties`. Godot's property system makes no distinction between script-defined exports and built-in node properties at the API level.

```json
{
  "op": "node_set_properties",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/enemy.tscn",
  "node_path": "Enemy",
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
  "source_path": "Enemy/DetectionZone",
  "signal_name": "body_entered",
  "target_path": "Enemy",
  "method_name": "_on_body_entered"
}
```

### Button press
```json
{
  "source_path": "UI/HUD/AttackButton",
  "signal_name": "pressed",
  "target_path": "Player",
  "method_name": "_on_attack_button_pressed"
}
```

### Timer timeout
```json
{
  "source_path": "Enemy/AttackCooldownTimer",
  "signal_name": "timeout",
  "target_path": "Enemy",
  "method_name": "_on_attack_cooldown_timeout"
}
```

### AnimationPlayer finished
```json
{
  "source_path": "Player/AnimationPlayer",
  "signal_name": "animation_finished",
  "target_path": "Player",
  "method_name": "_on_animation_finished"
}
```

## Tips

**Check existing connections before adding new ones.** Use `signal_list` to avoid creating duplicate connections — Godot will error or double-fire if the same connection is added twice.

**The `target_path` method must exist.** Director validates that the `target_path` has a script, but not that the method exists in that script. A missing method will cause a runtime error when the signal fires. Double-check method names.

**Use `spatial_inspect` with `include: ["signals"]`** to see connections in the running game. This is the fastest way to verify that wiring applied correctly.

**`node_set_properties` works for `@export` variables too.** Things like enemy health, damage, speed, and range are usually `@export` variables — set them with `node_set_properties` just like any built-in property.
