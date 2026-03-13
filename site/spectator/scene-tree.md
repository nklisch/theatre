<script setup>
const messages0 = [
  { role: 'human', text: `I am new to this project. What does the scene look like?` },
  { role: 'agent', text: `The scene has 4 top-level groups: Player (CharacterBody3D with 5 children including Camera3D and AnimationPlayer), Enemies (Node3D containing 3 CharacterBody3D enemies, each with NavigationAgent3D and an Area3D detection zone), Level (StaticBody3D ground, walls, and a TileMap), and UI (CanvasLayer with HUD). There are 47 total nodes.` },
]
const messages1 = [
  { role: 'human', text: `What is the full path to the gun's muzzle flash effect?` },
  { role: 'agent', text: `The muzzle flash is at: Player/WeaponPivot/MuzzleFlash (class: GPUParticles3D). Full path from root would be World/Player/WeaponPivot/MuzzleFlash.` },
]
const messages2 = [
  { role: 'human', text: `Show me all the enemy collision layers at a glance.` },
  { role: 'agent', text: `Enemy_0: layer=2, mask=1. Enemy_1: layer=2, mask=1. Enemy_2: layer=2, mask=0. Enemy_2 has collision_mask=0 — it will not detect the player.` },
]
</script>

# scene_tree

Get the scene tree structure without spatial data.

`scene_tree` returns the hierarchical layout of your running game's scene tree — node names, classes, and parent/child relationships. It does not include positions, velocities, or other spatial properties. This makes it much more compact than `spatial_snapshot` and ideal for understanding scene structure.

## When to use it

- **Understanding scene organization**: what nodes exist and how they are nested
- **Finding node paths**: "What is the full path to the animation player?"
- **Verifying Director changes**: "Did the node_add create the right hierarchy?"
- **Orientation in an unfamiliar project**: get a map before diving into details
- **Counting node types**: "How many enemies are in the scene?"

Use `spatial_snapshot` when you need spatial data. Use `scene_tree` when you need structure.

## Parameters

| Parameter | Type | Default | Description |
|---|---|---|---|
| `root` | `string` | `"/"` | Start from this node's subtree |
| `max_depth` | `integer` | `5` | Maximum depth of tree to return |
| `find_by` | `string` | `null` | Filter nodes by property name |
| `find_value` | `any` | `null` | Match value for `find_by` |
| `show_properties` | `string[]` | `null` | Include these properties inline (compact form) |

### `root`

Limit the tree to a subtree. For example, `"root": "Player"` returns only the player's hierarchy:

```json
{
  "root": "Player",
  "max_depth": 3
}
```

### `max_depth`

Controls tree depth. Depth 1 returns only the root's direct children; depth 5 returns 5 levels. For large scenes, keep depth low (2-3) to avoid enormous responses.

### `show_properties`

Add a few key properties inline without switching to `spatial_snapshot`:

```json
{
  "max_depth": 3,
  "show_properties": ["class", "visible", "collision_layer"]
}
```

## Response format

```json
{
  "root": "/",
  "frame": 450,
  "node_count": 47,
  "tree": {
    "name": "World",
    "class": "Node3D",
    "children": [
      {
        "name": "Player",
        "class": "CharacterBody3D",
        "children": [
          { "name": "CollisionShape3D", "class": "CollisionShape3D", "children": [] },
          { "name": "MeshInstance3D", "class": "MeshInstance3D", "children": [] },
          { "name": "Camera3D", "class": "Camera3D", "children": [] },
          { "name": "AnimationPlayer", "class": "AnimationPlayer", "children": [] },
          {
            "name": "WeaponPivot",
            "class": "Node3D",
            "children": [
              { "name": "Gun", "class": "MeshInstance3D", "children": [] },
              { "name": "MuzzleFlash", "class": "GPUParticles3D", "children": [] }
            ]
          }
        ]
      },
      {
        "name": "Enemies",
        "class": "Node3D",
        "children": [
          {
            "name": "Enemy_0",
            "class": "CharacterBody3D",
            "children": [
              { "name": "EnemyDetectionZone", "class": "Area3D", "children": [] },
              { "name": "NavigationAgent3D", "class": "NavigationAgent3D", "children": [] }
            ]
          }
        ]
      },
      {
        "name": "Level",
        "class": "Node3D",
        "children": [
          { "name": "Ground", "class": "StaticBody3D", "children": [] },
          { "name": "Walls", "class": "StaticBody3D", "children": [] },
          { "name": "TileMap", "class": "TileMap", "children": [] }
        ]
      },
      {
        "name": "UI",
        "class": "CanvasLayer",
        "children": [
          { "name": "HUD", "class": "Control", "children": [] }
        ]
      }
    ]
  }
}
```

### With `show_properties`

```json
{
  "root": "Enemies",
  "max_depth": 2,
  "show_properties": ["collision_layer", "collision_mask"]
}
```

Response includes inline properties:

```json
{
  "tree": {
    "name": "Enemies",
    "class": "Node3D",
    "children": [
      {
        "name": "Enemy_0",
        "class": "CharacterBody3D",
        "collision_layer": 2,
        "collision_mask": 1,
        "children": [
          {
            "name": "EnemyDetectionZone",
            "class": "Area3D",
            "collision_layer": 2,
            "collision_mask": 0,
            "children": []
          }
        ]
      }
    ]
  }
}
```

## Example conversations

### Understanding scene structure

<AgentConversation :messages="messages0" />

### Finding a node path

<AgentConversation :messages="messages1" />

### Scanning collision configuration

<AgentConversation :messages="messages2" />

## Tips

**Use `max_depth: 2-3` for large scenes.** Deep trees with many nodes can still produce large responses. Limit depth until you know where to drill.

**Use `root` to scope to a subsystem.** If debugging enemies, `root: "Enemies"` gives you only the enemy hierarchy without the full scene.

**`show_properties` is a quick audit tool.** If you want to check one specific property across many nodes (like `visible` or `collision_layer`), `show_properties` is more efficient than calling `spatial_inspect` on each node.

**Scene tree paths use node names, not indices.** The path `"Enemies/Enemy_0"` refers to the node named `Enemy_0` inside `Enemies`, not the first child. If two nodes have the same name, Godot appends `@2` — this appears in the tree response.
