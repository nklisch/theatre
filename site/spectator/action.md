<script setup>
import { data } from '../.vitepress/data/tools.data'
const params = data.params['spatial_action'] ?? []

const messages0 = [
  { role: 'human', text: `I think the collision_mask on the detection zone is wrong. Can you test setting it to 1 right now without restarting?` },
  { role: 'agent', text: `Done. collision_mask is now 1. Walk the player through the detection zone and see if the enemy alerts.` },
  { role: 'human', text: `It worked! The enemy detects me now. Can you now use Director to make this permanent?` },
  { role: 'agent', text: `Great. Setting the permanent fix via Director now.` },
  { role: 'agent', text: `Permanent fix applied. The scene file is updated. The collision_mask: 1 will persist after restart.` },
]
</script>

# spatial_action

Set properties, call methods, or emit signals on running game nodes.

`spatial_action` is the one Spectator tool that modifies game state. It is designed for testing — applying a quick change to verify a hypothesis or trigger an event without restarting the game.

## When to use it

- **Testing a fix hypothesis**: "If I set `collision_mask = 1`, does detection work?"
- **Triggering events for testing**: emit a signal, call `take_damage(50)`
- **Resetting state mid-session**: teleport player to a spawn point, reset health to 100
- **Verifying a Director change**: after Director updates a property, use action to verify at runtime

Changes made via `spatial_action` are **not persistent** — they affect the current game session only. When the game restarts, all changes are lost. For persistent changes, use Director.

## Parameters

<ParamTable :params="params" />

## Action types

### `set_property`

Set any accessible property on the node:

```json
{
  "node": "Player",
  "action": "set_property",
  "property": "collision_mask",
  "value": 3
}
```

```json
{
  "node": "Player",
  "action": "set_property",
  "property": "global_position",
  "value": [0.0, 1.0, 0.0]
}
```

```json
{
  "node": "Player",
  "action": "set_property",
  "property": "health",
  "value": 100
}
```

### `call_method`

Call any public method on the node:

```json
{
  "node": "Player",
  "action": "call_method",
  "method": "take_damage",
  "args": [50, "fire"]
}
```

```json
{
  "node": "Enemy_0",
  "action": "call_method",
  "method": "set_target",
  "args": ["Player"]
}
```

```json
{
  "node": "Player/AnimationPlayer",
  "action": "call_method",
  "method": "play",
  "args": ["attack"]
}
```

### `emit_signal`

Emit a signal on the node:

```json
{
  "node": "EventBus",
  "action": "emit_signal",
  "signal": "level_complete",
  "signal_args": [3, "A"]
}
```

```json
{
  "node": "Player",
  "action": "emit_signal",
  "signal": "died"
}
```

## Response format

```json
{
  "node": "Player",
  "action": "set_property",
  "property": "collision_mask",
  "value": 3,
  "result": "ok"
}
```

For `call_method`, if the method returns a value:

```json
{
  "node": "Player",
  "action": "call_method",
  "method": "get_health",
  "result": "ok",
  "return_value": 75
}
```

### Error response

```json
{
  "node": "Player",
  "action": "set_property",
  "property": "nonexistent_property",
  "result": "error",
  "error": "Property 'nonexistent_property' not found on CharacterBody3D"
}
```

## Example conversation

<AgentConversation :messages="messages0" />

## Safety considerations

`spatial_action` executes code in the **running game process** on the main thread. This means:

- Setting an invalid property type can cause a Godot error (the game may crash or log an error)
- Calling methods with wrong argument types triggers Godot's type checking
- Emitting signals triggers all connected signal handlers immediately

The server does basic validation (node existence, action type), but cannot validate property types or method signatures before execution. Always verify the exact property name and type from `spatial_inspect` before setting.

## Tips

**Use `spatial_inspect` first to get property names.** Property names must match exactly what Godot expects. `"collision_mask"` is correct; `"collisionMask"` or `"mask"` will fail.

**Test hypotheses before applying Director fixes.** The workflow is: `spatial_action` to test the change at runtime → confirm it works → `director` to make it permanent.

**Call `take_damage()`/`heal()` to test combat balance.** Instead of taking damage manually in-game, call the damage method directly with specific values to test edge cases.

**Teleport with `set_property` on `global_position`.** Setting `global_position` to `[x, y, z]` instantly moves the node — useful for testing respawn behavior, trigger zones at specific positions, etc.
