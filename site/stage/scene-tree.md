<script setup>
import { data } from '../.vitepress/data/tools.data'
const params = data.params['scene_tree'] ?? []

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

<ParamTable :params="params" />

### Actions

| Action | Description |
|---|---|
| `"roots"` | Return all top-level nodes in the scene |
| `"children"` | Return direct children of `node` |
| `"subtree"` | Return the full subtree rooted at `node` (respects `depth`) |
| `"ancestors"` | Return the parent chain from `node` up to the root |
| `"find"` | Search for nodes matching `find_by` + `find_value` |

### `node`

Required for `children`, `subtree`, and `ancestors`. Specify the node by name or scene path:

```json
{
  "action": "subtree",
  "node": "Player",
  "depth": 3
}
```

### `depth`

Controls how many levels deep to return (default: 3). For large scenes, keep depth low (2-3) to avoid enormous responses.

### `find_by` and `find_value`

Used with `action: "find"` to locate nodes by name, class, group, or script:

```json
{
  "action": "find",
  "find_by": "class",
  "find_value": "NavigationAgent3D"
}
```

```json
{
  "action": "find",
  "find_by": "group",
  "find_value": "enemies"
}
```

### `include`

Controls which metadata is included per node (default: `["class", "groups"]`):

| Value | Description |
|---|---|
| `"class"` | Godot class name |
| `"groups"` | Godot groups the node belongs to |
| `"script"` | Attached script path |
| `"visible"` | Visibility state |
| `"process_mode"` | Process mode setting |

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
      }
    ]
  }
}
```

### With `include: ["class", "groups", "visible"]`

```json
{
  "action": "subtree",
  "node": "Enemies",
  "depth": 2,
  "include": ["class", "groups", "visible"]
}
```

Response includes the requested metadata inline:

```json
{
  "tree": {
    "name": "Enemies",
    "class": "Node3D",
    "children": [
      {
        "name": "Enemy_0",
        "class": "CharacterBody3D",
        "groups": ["enemies"],
        "visible": true,
        "children": [
          {
            "name": "EnemyDetectionZone",
            "class": "Area3D",
            "groups": [],
            "visible": true,
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

**Use `depth: 2-3` for large scenes.** Deep trees with many nodes can still produce large responses. Limit depth until you know where to drill.

**Use `action: "subtree"` with `node` to scope to a subsystem.** If debugging enemies, `node: "Enemies"` gives you only the enemy hierarchy without the full scene.

**Use `action: "find"` to locate nodes by class or group.** Finding all `NavigationAgent3D` nodes or all nodes in the `"enemies"` group is faster than scanning the whole tree manually.

**Use `include: ["visible"]` for quick visibility audits.** Checking visibility across many nodes is more efficient than calling `spatial_inspect` on each one.

**Scene tree paths use node names, not indices.** The path `"Enemies/Enemy_0"` refers to the node named `Enemy_0` inside `Enemies`, not the first child. If two nodes have the same name, Godot appends `@2` — this appears in the tree response.
