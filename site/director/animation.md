<script setup>
const messages0 = [
  { role: 'human', text: `Create a bounce animation on the coin: it should move up 2 meters and back over 0.6 seconds, looping.` },
  { role: 'agent', text: `Animation created (0.6s, ping_pong loop). Adding the position track.` },
  { role: 'agent', text: `Track added at index 0. Setting keyframes.` },
  { role: 'agent', text: `Done. The coin now has a float animation: position goes from [0,0,0] to [0,2,0] over 0.6 seconds with ease_in_out easing, ping_pong looping. Enable autoplay on the AnimationPlayer if you want it to start immediately.` },
]
</script>

# Animation

Create animations and keyframes for Godot's AnimationPlayer.

Director can create new animations, add property tracks, and set keyframes — building animations programmatically without the editor's animation timeline.

## Operations

### `animation_create`

Create a new animation in an AnimationPlayer.

```json
{
  "op": "animation_create",
  "project_path": "/home/user/my-game",
  "scene": "scenes/player.tscn",
  "node": "Player/AnimationPlayer",
  "animation_name": "bounce",
  "length": 0.5,
  "loop_mode": "loop"
}
```

| Parameter | Type | Description |
|---|---|---|
| `node` | `string` | Path to the AnimationPlayer node |
| `animation_name` | `string` | Name of the animation to create |
| `length` | `float` | Duration in seconds |
| `loop_mode` | `string` | `"none"`, `"loop"`, `"ping_pong"` |

**Response:**
```json
{
  "op": "animation_create",
  "animation_name": "bounce",
  "length": 0.5,
  "loop_mode": "loop",
  "result": "ok"
}
```

### `animation_add_track`

Add a property track to an animation. A track targets a specific node property.

```json
{
  "op": "animation_add_track",
  "project_path": "/home/user/my-game",
  "scene": "scenes/player.tscn",
  "node": "Player/AnimationPlayer",
  "animation_name": "bounce",
  "track_path": ".:position",
  "track_type": "property"
}
```

| Parameter | Type | Description |
|---|---|---|
| `animation_name` | `string` | Which animation to add the track to |
| `track_path` | `string` | Node path + property in Godot track format |
| `track_type` | `string` | `"property"`, `"method"`, `"audio"`, `"animation"` |

**Track path format**: `"NodePath:property_name"` where the NodePath is relative to the AnimationPlayer's root. `"."` means the animated node itself. Examples:
- `".:position"` — position of the AnimationPlayer's root
- `"MeshInstance3D:scale"` — scale of a sibling node
- `"../Player:velocity"` — velocity of the parent's Player child

**Response:**
```json
{
  "op": "animation_add_track",
  "animation_name": "bounce",
  "track_path": ".:position",
  "track_index": 0,
  "result": "ok"
}
```

The `track_index` is needed for `animation_set_key`.

### `animation_set_key`

Set a keyframe value on a track at a specific time.

```json
{
  "op": "animation_set_key",
  "project_path": "/home/user/my-game",
  "scene": "scenes/player.tscn",
  "node": "Player/AnimationPlayer",
  "animation_name": "bounce",
  "track_index": 0,
  "time": 0.0,
  "value": [0.0, 0.0, 0.0],
  "easing": "ease_in_out"
}
```

| Parameter | Type | Description |
|---|---|---|
| `animation_name` | `string` | Target animation |
| `track_index` | `integer` | Which track (from `animation_add_track` response) |
| `time` | `float` | Time in seconds within the animation |
| `value` | any | Keyframe value (type matches the property) |
| `easing` | `string` | `"linear"`, `"ease_in"`, `"ease_out"`, `"ease_in_out"` (optional) |

### `animation_play`

Trigger an animation to play on the running game (for testing). This calls AnimationPlayer.play() via the running GDExtension — it does not save anything to the scene.

```json
{
  "op": "animation_play",
  "project_path": "/home/user/my-game",
  "node": "Player/AnimationPlayer",
  "animation_name": "bounce",
  "speed_scale": 1.0
}
```

## Complete example: Creating a bounce animation

This creates an animation that moves a node from y=0 to y=2 and back, with easing.

<AgentConversation :messages="messages0" />

## Setting autoplay

To set an animation as the autoplay animation:

```json
{
  "op": "node_set_property",
  "project_path": "/home/user/my-game",
  "scene": "scenes/pickups.tscn",
  "node": "Coin/AnimationPlayer",
  "property": "autoplay",
  "value": "float"
}
```

## Tips

**Plan keyframes before calling Director.** For a smooth animation, sketch out the keyframe values and times first, then pass them in sequence. It is easier to think through the animation before starting the API calls.

**Use `ping_pong` for symmetric loops.** A bounce from 0→2 with `ping_pong` automatically returns 2→0. With `loop`, you need to manually set the return keyframe.

**Track paths are relative to the AnimationPlayer root.** The AnimationPlayer's root node (usually the scene root or the node the player is attached to) is `"."`. Siblings are `"SiblingName"`, children are `"ChildName"`.

**Test with `animation_play` before saving.** Play the animation via the running game first to check timing and feel. Only save to the scene file once satisfied.
