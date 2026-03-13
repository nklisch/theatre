<script setup>
import { data } from '../.vitepress/data/tools.data'

const animation_create = data.params['animation_create'] ?? []
const animation_add_track = data.params['animation_add_track'] ?? []

const messages0 = [
  { role: 'human', text: `Create a bounce animation on the coin: it should move up 2 meters and back over 0.6 seconds, looping.` },
  { role: 'agent', text: `Animation created (0.6s, ping_pong loop). Adding the position track with keyframes.` },
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

<ParamTable :params="animation_create" />

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

Add a property track to an animation, with all keyframes specified up front. A track targets a specific node property.

```json
{
  "op": "animation_add_track",
  "project_path": "/home/user/my-game",
  "scene": "scenes/player.tscn",
  "node": "Player/AnimationPlayer",
  "animation_name": "bounce",
  "track_path": ".:position",
  "track_type": "property",
  "keyframes": [
    { "time": 0.0, "value": [0.0, 0.0, 0.0], "easing": "ease_in_out" },
    { "time": 0.6, "value": [0.0, 2.0, 0.0], "easing": "ease_in_out" }
  ]
}
```

<ParamTable :params="animation_add_track" />

**Track path format**: `"NodePath:property_name"` where the NodePath is relative to the AnimationPlayer's root. `"."` means the animated node itself. Examples:
- `".:position"` — position of the AnimationPlayer's root
- `"MeshInstance3D:scale"` — scale of a sibling node
- `"../Player:velocity"` — velocity of the parent's Player child

**Keyframe `easing` values**: `"linear"`, `"ease_in"`, `"ease_out"`, `"ease_in_out"`

**Response:**
```json
{
  "op": "animation_add_track",
  "animation_name": "bounce",
  "track_path": ".:position",
  "track_index": 0,
  "keyframes_set": 2,
  "result": "ok"
}
```

## Complete example: Creating a bounce animation

This creates an animation that moves a node from y=0 to y=2 and back, with easing. All keyframes are passed directly in `animation_add_track`.

<AgentConversation :messages="messages0" />

## Setting autoplay

To set an animation as the autoplay animation:

```json
{
  "op": "node_set_properties",
  "project_path": "/home/user/my-game",
  "scene": "scenes/pickups.tscn",
  "node": "Coin/AnimationPlayer",
  "properties": {
    "autoplay": "float"
  }
}
```

## Tips

**Pass all keyframes in `animation_add_track`.** The `keyframes` array contains every keyframe for the track. There is no separate "set key" call — all timing and values are specified up front.

**Use `ping_pong` for symmetric loops.** A bounce from 0→2 with `ping_pong` automatically returns 2→0. With `loop`, you need to manually include the return keyframe in the `keyframes` array.

**Track paths are relative to the AnimationPlayer root.** The AnimationPlayer's root node (usually the scene root or the node the player is attached to) is `"."`. Siblings are `"SiblingName"`, children are `"ChildName"`.

**Use `spatial_action` to preview animations at runtime.** Call `AnimationPlayer.play("bounce")` via `spatial_action` to preview the animation in the running game without saving to the scene file.
