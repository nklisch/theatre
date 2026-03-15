<script setup>
import { data } from '../.vitepress/data/tools.data'

const animation_create = data.params['animation_create'] ?? []
const animation_add_track = data.params['animation_add_track'] ?? []
const animation_read = data.params['animation_read'] ?? []
const animation_remove_track = data.params['animation_remove_track'] ?? []

const messages0 = [
  { role: 'human', text: `Create a bounce animation on the coin: it should move up 2 meters and back over 0.6 seconds, looping.` },
  { role: 'agent', text: `Animation created (0.6s, pingpong loop). Adding the position track with keyframes.` },
  { role: 'agent', text: `Done. The coin now has a float animation: position goes from [0,0,0] to [0,2,0] over 0.6 seconds, pingpong looping. Assign this AnimationLibrary to an AnimationPlayer and enable autoplay to start it immediately.` },
]
</script>

# Animation

Create animations and keyframes for Godot's animation system.

Director works with animation resources (`.tres` files) directly. Animations are created as standalone resources that can be assigned to `AnimationPlayer` nodes via `AnimationLibrary`.

## Operations

### `animation_create`

Create a new animation resource and save it to disk.

```json
{
  "op": "animation_create",
  "project_path": "/home/user/my-game",
  "resource_path": "animations/coin_float.tres",
  "length": 0.6,
  "loop_mode": "pingpong"
}
```

<ParamTable :params="animation_create" />

**Response:**
```json
{
  "op": "animation_create",
  "resource_path": "animations/coin_float.tres",
  "length": 0.6,
  "loop_mode": "pingpong",
  "result": "ok"
}
```

### `animation_add_track`

Add a track to an animation resource, with all keyframes specified up front. A track targets a specific node and property (or transform component).

```json
{
  "op": "animation_add_track",
  "project_path": "/home/user/my-game",
  "resource_path": "animations/coin_float.tres",
  "track_type": "value",
  "node_path": ".:position",
  "keyframes": [
    { "time": 0.0, "value": [0.0, 0.0, 0.0], "transition": 1.0 },
    { "time": 0.6, "value": [0.0, 2.0, 0.0], "transition": 1.0 }
  ]
}
```

<ParamTable :params="animation_add_track" />

**Track types**:
- `"value"` — animate any property: `node_path` is `"NodePath:property_name"`
- `"position_3d"` / `"rotation_3d"` / `"scale_3d"` — transform tracks: `node_path` is the node path only
- `"blend_shape"` — blend shape weight: `node_path` is `"NodePath:blend_shape_name"`
- `"method"` — call a method at a time: keyframes use `method` and `args` fields
- `"bezier"` — bezier curve track: keyframes use `in_handle` and `out_handle`

**Node path format for `value` tracks**: `"NodePath:property_name"` where the NodePath is relative to the animation root. `"."` means the root node itself. Examples:
- `".:position"` — position of the root node
- `"MeshInstance3D:scale"` — scale of a child called MeshInstance3D
- `"../Player:velocity"` — velocity of a sibling

**`transition`**: Controls easing at the keyframe. `1.0` = linear, values < 1.0 ease in, values > 1.0 ease out.

**Response:**
```json
{
  "op": "animation_add_track",
  "resource_path": "animations/coin_float.tres",
  "track_index": 0,
  "keyframes_set": 2,
  "result": "ok"
}
```

### `animation_read`

Read the contents of an animation resource — tracks, keyframes, and metadata.

```json
{
  "op": "animation_read",
  "project_path": "/home/user/my-game",
  "resource_path": "animations/attack.tres"
}
```

<ParamTable :params="animation_read" />

**Response:**
```json
{
  "op": "animation_read",
  "resource_path": "animations/attack.tres",
  "length": 0.6,
  "loop_mode": "none",
  "tracks": [
    {
      "track_index": 0,
      "track_type": "value",
      "node_path": "WeaponPivot/HitArea:monitoring",
      "keyframes": [
        { "time": 0.0, "value": false },
        { "time": 0.1, "value": true },
        { "time": 0.4, "value": false }
      ]
    }
  ]
}
```

### `animation_remove_track`

Remove a track from an animation resource by index or node path.

```json
{
  "op": "animation_remove_track",
  "project_path": "/home/user/my-game",
  "resource_path": "animations/attack.tres",
  "track_index": 0
}
```

Or remove all tracks for a node path:

```json
{
  "op": "animation_remove_track",
  "project_path": "/home/user/my-game",
  "resource_path": "animations/attack.tres",
  "node_path": "WeaponPivot/HitArea:monitoring"
}
```

<ParamTable :params="animation_remove_track" />

## Complete example: Creating a bounce animation

This creates an animation that moves a node from y=0 to y=2 and back, with pingpong looping. All keyframes are passed directly in `animation_add_track`.

<AgentConversation :messages="messages0" />

## Setting autoplay

After assigning an animation resource to an `AnimationPlayer` via its library, set the autoplay property:

```json
{
  "op": "node_set_properties",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/pickups.tscn",
  "node_path": "Coin/AnimationPlayer",
  "properties": {
    "autoplay": "float"
  }
}
```

## Tips

**Pass all keyframes in `animation_add_track`.** The `keyframes` array contains every keyframe for the track. There is no separate "set key" call — all timing and values are specified up front.

**Use `pingpong` for symmetric loops.** A bounce from 0→2 with `pingpong` automatically returns 2→0. With `linear`, you need to manually include the return keyframe in the `keyframes` array.

**Use `animation_read` to inspect existing animations.** Before modifying keyframes, read the animation to get the exact `track_index` values and current keyframe times.

**Node paths are relative to the animation root.** The root node is `"."`. Siblings are `"SiblingName"`, children are `"ChildName"`.
