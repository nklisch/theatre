---
description: "Spatial Action lets AI agents call methods and set properties on running game nodes to test fixes without restarting."
---

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

Interact with the running game: control execution, modify node state, inject input, and spawn or remove nodes.

`spatial_action` is the one Stage tool that modifies game state. It covers a broad set of actions: pausing and stepping the game, teleporting nodes, setting properties, calling methods, emitting signals, spawning scenes, and injecting keyboard and mouse input.

## When to use it

- **Testing a fix hypothesis**: "If I set `collision_mask = 1`, does detection work?"
- **Triggering events for testing**: emit a signal, call `take_damage(50)`
- **Controlling execution**: pause the game, step frame-by-frame to inspect a bug
- **Teleporting nodes**: move the player to a trigger zone without playing through
- **Spawning nodes**: instantiate a prefab for testing without restarting
- **Input injection**: simulate a button press to trigger an action in code
- **Verifying a Director change**: after Director updates a property, use action to verify at runtime

Changes made via `spatial_action` are **not persistent** — they affect the current game session only. When the game restarts, all changes are lost. For persistent changes, use Director.

## Parameters

<ParamTable :params="params" />

## Action types

### `pause`

Pause or unpause the game:

```json
{
  "action": "pause",
  "paused": true
}
```

```json
{
  "action": "pause",
  "paused": false
}
```

Pausing is useful before calling `advance_frames` or `advance_time` to step through a bug one frame at a time.

### `advance_frames`

Step the game forward by N physics frames while paused:

```json
{
  "action": "advance_frames",
  "frames": 5
}
```

The game must be paused before calling this. Each step runs physics and script logic for that frame. Combine with `spatial_snapshot` after each step to observe the exact state at each frame.

### `advance_time`

Step the game forward by N seconds while paused:

```json
{
  "action": "advance_time",
  "seconds": 0.5
}
```

Advances the physics simulation by the specified duration. Useful when you need to move forward by a specific time rather than a frame count.

### `teleport`

Move a node to a position immediately:

```json
{
  "action": "teleport",
  "node": "Player",
  "position": [10.0, 0.0, -5.0]
}
```

Optionally set the Y rotation at the same time:

```json
{
  "action": "teleport",
  "node": "Player",
  "position": [10.0, 0.0, -5.0],
  "rotation_deg": 90.0
}
```

This sets `global_position` directly. Useful for placing the player at a trigger zone, a spawn point, or a specific location to reproduce a bug.

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
  "args": [3, "A"]
}
```

```json
{
  "node": "Player",
  "action": "emit_signal",
  "signal": "died"
}
```

### `spawn_node`

Instantiate a scene as a child node in the running game:

```json
{
  "action": "spawn_node",
  "scene_path": "res://enemies/goblin.tscn",
  "parent": "Enemies",
  "name": "Goblin_Test"
}
```

The `scene_path` must be a valid `res://` path. `parent` is the node that will own the new instance. `name` is optional — Godot assigns one if omitted.

### `remove_node`

Remove a node from the scene tree:

```json
{
  "action": "remove_node",
  "node": "Enemies/Goblin_Test"
}
```

The node and all its children are freed immediately. Use this to clean up spawned test nodes.

### `action_press`

Simulate an input action press (as defined in Godot's Input Map):

```json
{
  "action": "action_press",
  "input_action": "jump"
}
```

```json
{
  "action": "action_press",
  "input_action": "fire",
  "strength": 0.8
}
```

`strength` is optional, defaulting to 1.0. The action stays pressed until `action_release` is called or the game processes its own input.

### `action_release`

Release a previously-pressed input action:

```json
{
  "action": "action_release",
  "input_action": "jump"
}
```

### `inject_key`

Simulate a keyboard key event:

```json
{
  "action": "inject_key",
  "keycode": "Space",
  "pressed": true
}
```

```json
{
  "action": "inject_key",
  "keycode": "Escape",
  "pressed": false,
  "echo": false
}
```

`keycode` uses Godot's key name strings (`Space`, `Enter`, `Escape`, `F1`, `A`–`Z`, etc.). `pressed` defaults to `true`. `echo` simulates a held-key repeat event.

### `inject_mouse_button`

Simulate a mouse button click:

```json
{
  "action": "inject_mouse_button",
  "button": "Left",
  "pressed": true,
  "position": [640.0, 360.0]
}
```

`button` is `"Left"`, `"Right"`, or `"Middle"`. `position` is in screen coordinates. `pressed` defaults to `true`.

## The `return_delta` parameter

Any action can include `"return_delta": true` to append a `spatial_delta` to the response. This is useful when you want to see the immediate effect of an action without making a separate delta call:

```json
{
  "action": "teleport",
  "node": "Player",
  "position": [10.0, 0.0, -5.0],
  "return_delta": true
}
```

The response will include the action result plus a `delta` field showing what changed.

## Response format

```json
{
  "action": "set_property",
  "result": "ok"
}
```

For `call_method`, if the method returns a value:

```json
{
  "action": "call_method",
  "result": "ok",
  "return_value": 75
}
```

### Error response

```json
{
  "action": "set_property",
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
- Input injection affects game input just as if the player pressed the key

The server does basic validation (node existence, action type), but cannot validate property types or method signatures before execution. Always verify the exact property name and type from `spatial_inspect` before setting.

## Tips

**Use `spatial_inspect` first to get property names.** Property names must match exactly what Godot expects. `"collision_mask"` is correct; `"collisionMask"` or `"mask"` will fail.

**Pause then step frame-by-frame for timing bugs.** Call `pause: true`, then `advance_frames: 1` repeatedly, taking a snapshot after each step to see exactly how state evolves.

**Test hypotheses before applying Director fixes.** The workflow is: `spatial_action` to test the change at runtime → confirm it works → `director` to make it permanent.

**Use `return_delta: true` to verify actions immediately.** Instead of calling `spatial_action` then `spatial_delta` separately, set `return_delta: true` to get both in one round-trip.

**`spawn_node` + `remove_node` for quick instantiation tests.** Spawn a scene at runtime, observe how it interacts with the level, then remove it — all without restarting the game.
