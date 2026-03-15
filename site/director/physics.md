---
description: "Physics layer management — configure collision layers, masks, and physics material properties through Director."
---

<script setup>
import { data } from '../.vitepress/data/tools.data'

const physics_set_layer_names = data.params['physics_set_layer_names'] ?? []
const physics_set_layers = data.params['physics_set_layers'] ?? []

const messages0 = [
  { role: 'human', text: `Bullets are not hitting enemies. I think the collision layers are wrong. Check and fix them.` },
  { role: 'agent', text: `Enemy: collision_layer=2, collision_mask=1. Bullet scene: the Area3D has collision_layer=8 (layer 4), collision_mask=0. The bullet Area3D has mask=0 — it monitors zero layers, so it will never detect anything. Set bullet Area3D collision_mask to 2 (enemy layer).` },
  { role: 'agent', text: `Fixed. Bullet HitArea now monitors layer 2 (Enemy). The body_entered signal will fire when the bullet overlaps an enemy. Make sure the signal is connected to a hit handler in the bullet script.` },
]
</script>

# Physics Layers

Configure collision layers, masks, and layer names for your project.

Godot's physics layer system uses bitmasks with 32 layers. Each layer has a number (1-32) and can have a human-readable name. Getting these right is critical for correct collision detection — wrong layers are one of the most common sources of "why doesn't this collide?" bugs.

## Understanding the layer system

Godot physics bodies have two bitmask properties:

- **`collision_layer`**: "I am on these layers" — which layers this body occupies
- **`collision_mask`**: "I detect these layers" — which layers this body can interact with

Two bodies interact if **body A's mask includes body B's layer** (or vice versa for mutual detection). This is an AND relationship: the bits must overlap.

Example:
```
Player:  layer=0b0001 (layer 1), mask=0b0110 (layers 2+3)
Enemy:   layer=0b0010 (layer 2), mask=0b0001 (layer 1)
Wall:    layer=0b0100 (layer 3), mask=0b0000 (no detection)
```

Player detects Enemies (player mask has layer 2 set) and Walls (player mask has layer 3 set). Enemies detect Players. Walls detect nothing. Enemies do not detect Walls (enemy mask=1, wall layer=4, no overlap).

## Operations

### `physics_set_layers`

Set `collision_layer` and/or `collision_mask` bitmasks on a node.

```json
{
  "op": "physics_set_layers",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/player.tscn",
  "node_path": "Player",
  "collision_layer": 1,
  "collision_mask": 6
}
```

<ParamTable :params="physics_set_layers" />

**Response:**
```json
{
  "op": "physics_set_layers",
  "node_path": "Player",
  "collision_layer": 1,
  "collision_mask": 6,
  "result": "ok"
}
```

### `physics_set_layer_names`

Set the display names for a layer type. Names appear in the Godot editor's layer picker.

```json
{
  "op": "physics_set_layer_names",
  "project_path": "/home/user/my-game",
  "layer_type": "3d_physics",
  "layers": {
    "1": "Player",
    "2": "Enemies",
    "3": "World",
    "4": "Pickups",
    "5": "Projectiles",
    "6": "Triggers"
  }
}
```

`layer_type` values: `"2d_physics"`, `"3d_physics"`, `"2d_render"`, `"3d_render"`, `"2d_navigation"`, `"3d_navigation"`, `"avoidance"`.

<ParamTable :params="physics_set_layer_names" />

## Layer number to bitmask conversion

| Layers active | Bitmask |
|---|---|
| Layer 1 | `1` |
| Layer 2 | `2` |
| Layers 1+2 | `3` |
| Layer 3 | `4` |
| Layers 1+3 | `5` |
| Layers 2+3 | `6` |
| Layers 1+2+3 | `7` |
| Layer 4 | `8` |

The formula: bitmask = sum of (2^(layer_number - 1)) for each active layer.

Director accepts both bitmask integers and layer number arrays — use whichever is clearer.

## Example conversation: Fixing a detection problem

<AgentConversation :messages="messages0" />

## Tips

**Name your layers before assigning them.** Use `physics_set_layer_names` to set up a consistent naming scheme (Player, Enemies, World, Projectiles, Triggers). This makes layer assignments readable — "layer 2" means nothing; "Enemies" is clear.

**Use the bitmask directly.** Pass `"collision_layer": 5` (layers 1+3) rather than computing it separately. The formula is: sum of (2^(layer_number - 1)) for each active layer.

**Layers are 1-indexed in Director, but 0-indexed internally in Godot.** When reading collision_layer bitmasks in Godot's GDScript, bit 0 = layer 1. Director uses 1-indexed layer numbers to avoid off-by-one confusion.

**Area3D requires `monitoring: true`.** An Area3D with correct collision layers but `monitoring=false` will never emit body_entered/area_entered. Check this with `spatial_inspect` if detection is still failing after fixing layers.
